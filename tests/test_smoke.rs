//! Prueba de extremo a extremo del binario: analiza y escribe una imagen.

use std::process::Command;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("photo_studio_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn analiza_y_procesa_una_imagen() {
    let dir = temp_dir("e2e");
    let input = dir.join("test.png");
    let img = image::RgbImage::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 3) as u8, (y * 3) as u8, 90])
    });
    img.save(&input).unwrap();

    let dry = Command::new(env!("CARGO_BIN_EXE_photo_studio"))
        .arg(&input)
        .arg("--dry-run")
        .output()
        .unwrap();
    assert!(dry.status.success());
    assert!(String::from_utf8_lossy(&dry.stdout).contains("Y="));

    let out = dir.join("out");
    let run = Command::new(env!("CARGO_BIN_EXE_photo_studio"))
        .arg(&input)
        .arg("-o")
        .arg(&out)
        .arg("--long-side")
        .arg("64")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let produced = out.join("test_studio.jpg");
    assert!(produced.exists());
    let decoded = image::open(&produced).unwrap();
    assert_eq!(decoded.width(), 64);
    assert_eq!(decoded.height(), 64);
    let _ = std::fs::remove_dir_all(&dir);
}
