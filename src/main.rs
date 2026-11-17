//! photo_studio: mejora por lotes fotos de retrato para que una IA generativa
//! las edite sin perder la cara.
//!
//! Uso:
//!   photo_studio <archivos o carpetas>... [-o salida] [--long-side 2560]
//!                [--strength 1.0] [--dry-run]
//!
//! Limitaciones: no recupera detalle que la cámara no capturó y no corrige la
//! orientación EXIF (las fotos de móvil ya suelen venir derechas).

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::Parser;

mod guided;
mod ops;
mod plan;
mod stats;

#[derive(Parser, Debug)]
#[command(
    name = "photo_studio",
    version,
    about = "Mejora fotos de retrato por lotes: color, tono, textura y tamaño para edición con IA."
)]
struct Args {
    /// Archivos o carpetas de entrada.
    #[arg(required = true)]
    inputs: Vec<PathBuf>,

    /// Carpeta de salida.
    #[arg(short, long, default_value = "studio")]
    out: PathBuf,

    /// Lado largo de salida en píxeles (0 = dejar el original).
    #[arg(long, default_value_t = 2560)]
    long_side: u32,

    /// Fuerza global de los ajustes, de 0.0 a 1.5.
    #[arg(long, default_value_t = 1.0)]
    strength: f32,

    /// Solo medir y mostrar el diagnóstico, sin escribir archivos.
    #[arg(long)]
    dry_run: bool,
}

fn is_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("jpg" | "jpeg" | "png")
    )
}

fn collect(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut files = Vec::new();
    for input in inputs {
        if input.is_dir() {
            for entry in std::fs::read_dir(input)? {
                let path = entry?.path();
                if path.is_file() && is_image(&path) {
                    files.push(path);
                }
            }
        } else if input.is_file() {
            files.push(input.clone());
        } else {
            return Err(format!("no existe: {}", input.display()).into());
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn process(path: &Path, args: &Args) -> Result<(), Box<dyn Error>> {
    let img = image::ImageReader::open(path)?
        .with_guessed_format()?
        .decode()?
        .to_rgb8();
    let (w, h) = img.dimensions();
    let measured = stats::analyze(&img);
    let settings = plan::plan(&measured, args.strength, w.max(h));

    println!(
        "{:<34} {:>4}x{:<4}  Y={:>3}  sat={:>3}  nit={:>5.2}  ruido={:>4.2}  ->  wb {:.2}/{:.2}/{:.2}  g={:.2}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        w,
        h,
        (measured.mean * 255.0).round(),
        (measured.saturation * 255.0).round(),
        measured.sharpness * 255.0,
        measured.noise,
        settings.wb[0],
        settings.wb[1],
        settings.wb[2],
        settings.gamma,
    );

    if args.dry_run {
        return Ok(());
    }

    let mut out = ops::apply(&img, &settings)?;
    out = ops::resize_long_side(&out, args.long_side);
    ops::sharpen(&mut out, settings.sharpen);

    std::fs::create_dir_all(&args.out)?;
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let dest = args.out.join(format!("{stem}_studio.jpg"));
    let (ow, oh) = out.dimensions();
    let mut encoder = jpeg_encoder::Encoder::new_file(&dest, 95)?;
    encoder.set_sampling_factor(jpeg_encoder::SamplingFactor::R_4_4_4);
    encoder.encode(
        out.as_raw(),
        ow as u16,
        oh as u16,
        jpeg_encoder::ColorType::Rgb,
    )?;
    println!("    -> {} ({}x{})", dest.display(), ow, oh);
    Ok(())
}

fn main() {
    let args = Args::parse();
    if !(0.0..=1.5).contains(&args.strength) {
        eprintln!("--strength debe estar entre 0.0 y 1.5");
        std::process::exit(2);
    }

    let files = match collect(&args.inputs) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    if files.is_empty() {
        eprintln!("no encontré imágenes jpg/png en la entrada");
        std::process::exit(1);
    }

    let start = Instant::now();
    let mut failed = 0;
    for file in &files {
        if let Err(e) = process(file, &args) {
            eprintln!("error en {}: {e}", file.display());
            failed += 1;
        }
    }
    println!(
        "\n{} imagen(es) en {:.1} s{}",
        files.len() - failed,
        start.elapsed().as_secs_f32(),
        if failed > 0 {
            format!(", {failed} con error")
        } else {
            String::new()
        }
    );
    if failed > 0 {
        std::process::exit(1);
    }
}
