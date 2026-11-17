//! Medición de la imagen: exposición, balance de blancos, saturación, nitidez y ruido.

use image::RgbImage;

/// Pesos de luminancia Rec. 709.
pub(crate) const REC709: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// Luminancia de un color en 0..1.
pub(crate) fn luma(r: f32, g: f32, b: f32) -> f32 {
    r * REC709[0] + g * REC709[1] + b * REC709[2]
}

/// Medidas de una foto, todas normalizadas a 0..1 salvo `noise`, que va en niveles de 8 bits.
#[derive(Debug, Clone, Default)]
pub struct Stats {
    /// Luminancia media.
    pub mean: f32,
    /// Percentil 0,5 de luminancia (punto negro).
    pub lo: f32,
    /// Percentil 99,5 de luminancia (punto blanco).
    pub hi: f32,
    /// Mediana de luminancia en la zona central, donde suele estar la cara.
    pub center_median: f32,
    /// Saturación media (HSV, 0..1).
    pub saturation: f32,
    /// Laplaciano medio: más alto es más nitidez.
    pub sharpness: f32,
    /// Ruido de altas frecuencias en las zonas planas, en niveles de 8 bits.
    pub noise: f32,
    /// Medias por canal.
    pub mean_rgb: [f32; 3],
}

struct Exposure {
    hist: [u32; 256],
    center_hist: [u32; 256],
    center_n: u64,
    mean: f32,
    mean_rgb: [f32; 3],
    saturation: f32,
}

fn percentile(hist: &[u32; 256], total: u64, p: f32) -> f32 {
    let target = (total as f32 * p).ceil().max(1.0) as u64;
    let mut acc = 0u64;
    for (i, &n) in hist.iter().enumerate() {
        acc += n as u64;
        if acc >= target {
            return i as f32 / 255.0;
        }
    }
    1.0
}

fn exposure(img: &RgbImage) -> Exposure {
    let (w, h) = img.dimensions();
    let src = img.as_raw();
    let mut e = Exposure {
        hist: [0u32; 256],
        center_hist: [0u32; 256],
        center_n: 0,
        mean: 0.0,
        mean_rgb: [0.0; 3],
        saturation: 0.0,
    };
    let mut sum = [0f64; 3];
    let mut sat_sum = 0f64;
    let mut sat_n = 0u64;
    let cx0 = (w as f32 * 0.25) as u32;
    let cx1 = (w as f32 * 0.75) as u32;
    let cy0 = (h as f32 * 0.15) as u32;
    let cy1 = (h as f32 * 0.55) as u32;

    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 3) as usize;
            let r = src[i] as f32 / 255.0;
            let g = src[i + 1] as f32 / 255.0;
            let b = src[i + 2] as f32 / 255.0;
            sum[0] += r as f64;
            sum[1] += g as f64;
            sum[2] += b as f64;
            let yv = luma(r, g, b);
            e.mean += yv;
            let idx = (yv * 255.0).round().clamp(0.0, 255.0) as usize;
            e.hist[idx] += 1;
            if x >= cx0 && x < cx1 && y >= cy0 && y < cy1 {
                e.center_hist[idx] += 1;
                e.center_n += 1;
            }
            if x % 2 == 0 && y % 2 == 0 {
                let mx = r.max(g).max(b);
                let mn = r.min(g).min(b);
                if mx > 1e-4 {
                    sat_sum += ((mx - mn) / mx) as f64;
                    sat_n += 1;
                }
            }
        }
    }

    let n = (w as u64) * (h as u64);
    e.mean /= n.max(1) as f32;
    e.mean_rgb = [
        (sum[0] / n.max(1) as f64) as f32,
        (sum[1] / n.max(1) as f64) as f32,
        (sum[2] / n.max(1) as f64) as f32,
    ];
    e.saturation = if sat_n > 0 {
        (sat_sum / sat_n as f64) as f32
    } else {
        0.0
    };
    e
}

fn sharpness(img: &RgbImage) -> f32 {
    let (w, h) = img.dimensions();
    let src = img.as_raw();
    let cx0 = (w as f32 * 0.25) as u32;
    let cx1 = (w as f32 * 0.75) as u32;
    let cy0 = (h as f32 * 0.2) as u32;
    let cy1 = (h as f32 * 0.8) as u32;
    let mut lap = 0f64;
    let mut n = 0u64;
    let at = |x: u32, y: u32| {
        let i = ((y * w + x) * 3) as usize;
        luma(
            src[i] as f32 / 255.0,
            src[i + 1] as f32 / 255.0,
            src[i + 2] as f32 / 255.0,
        )
    };
    for y in (cy0..cy1).step_by(4) {
        for x in (cx0..cx1).step_by(4) {
            let c = at(x, y);
            let l = at(x.saturating_sub(1), y);
            let r = at((x + 1).min(w - 1), y);
            let u = at(x, y.saturating_sub(1));
            let d = at(x, (y + 1).min(h - 1));
            lap += (4.0 * c - l - r - u - d).abs() as f64;
            n += 1;
        }
    }
    if n > 0 { (lap / n as f64) as f32 } else { 0.0 }
}

fn noise(img: &RgbImage) -> f32 {
    let (w, h) = img.dimensions();
    let src = img.as_raw();
    let mut blocks = Vec::new();
    let mut by = 0;
    while by + 8 <= h {
        let mut bx = 0;
        while bx + 8 <= w {
            let mut vals = [0f32; 64];
            for yy in 0..8 {
                for xx in 0..8 {
                    let i = (((by + yy) * w + bx + xx) * 3) as usize;
                    vals[(yy * 8 + xx) as usize] = luma(
                        src[i] as f32 / 255.0,
                        src[i + 1] as f32 / 255.0,
                        src[i + 2] as f32 / 255.0,
                    );
                }
            }
            // Solo el interior 6x6: cada píxel contra la media 3x3 del bloque.
            let mut hp2 = 0f32;
            for yy in 1..7 {
                for xx in 1..7 {
                    let mut avg = 0f32;
                    for dy in 0..3 {
                        for dx in 0..3 {
                            avg += vals[(yy + dy - 1) * 8 + (xx + dx - 1)];
                        }
                    }
                    let d = vals[yy * 8 + xx] - avg / 9.0;
                    hp2 += d * d;
                }
            }
            blocks.push((hp2 / 36.0).sqrt());
            bx += 16;
        }
        by += 16;
    }
    blocks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if blocks.is_empty() {
        0.0
    } else {
        blocks[(blocks.len() as f32 * 0.2) as usize] * 255.0
    }
}

/// Mide la foto y devuelve sus estadísticas.
pub fn analyze(img: &RgbImage) -> Stats {
    let e = exposure(img);
    let (w, h) = img.dimensions();
    let n = (w as u64) * (h as u64);
    Stats {
        mean: e.mean,
        lo: percentile(&e.hist, n, 0.005),
        hi: percentile(&e.hist, n, 0.995),
        center_median: percentile(&e.center_hist, e.center_n.max(1), 0.5),
        saturation: e.saturation,
        sharpness: sharpness(img),
        noise: noise(img),
        mean_rgb: e.mean_rgb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn gris_plano_no_tiene_ruido_ni_saturacion() {
        let img = RgbImage::from_pixel(32, 32, Rgb([100, 100, 100]));
        let s = analyze(&img);
        assert!(s.noise < 0.01, "ruido {}", s.noise);
        assert!(s.saturation < 0.01, "sat {}", s.saturation);
        assert!((s.mean - 100.0 / 255.0).abs() < 0.01);
    }
}
