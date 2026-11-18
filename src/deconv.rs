//! Deconvolución Richardson-Lucy sobre luminancia.
//!
//! Recupera foco de una imagen óptimamente blanda. No inventa rasgos: reparte
//! la energía de los bordes según una PSF gaussiana y el propio dato. Es más
//! honesta que un unsharp mask, pero amplifica ruido, así que va después del
//! denoise y con pocas iteraciones.

const KERNEL: [f32; 7] = [0.0147, 0.0830, 0.2357, 0.3333, 0.2357, 0.0830, 0.0147];

fn blur(src: &[f32], dst: &mut [f32], tmp: &mut [f32], w: usize, h: usize) {
    let r = KERNEL.len() / 2;
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        for x in 0..w {
            let mut sum = 0.0;
            for (i, &k) in KERNEL.iter().enumerate() {
                let sx = (x + i).saturating_sub(r).min(w - 1);
                sum += row[sx] * k;
            }
            tmp[y * w + x] = sum;
        }
    }
    for x in 0..w {
        for y in 0..h {
            let mut sum = 0.0;
            for (i, &k) in KERNEL.iter().enumerate() {
                let sy = (y + i).saturating_sub(r).min(h - 1);
                sum += tmp[sy * w + x] * k;
            }
            dst[y * w + x] = sum;
        }
    }
}

/// Ejecuta `iters` iteraciones de Richardson-Lucy. `src` va en 0..1.
pub(crate) fn richardson_lucy(src: &[f32], w: usize, h: usize, iters: u32) -> Vec<f32> {
    if iters == 0 {
        return src.to_vec();
    }
    let mut est = src.to_vec();
    let mut conv = vec![0.0f32; src.len()];
    let mut ratio = vec![0.0f32; src.len()];
    let mut corr = vec![0.0f32; src.len()];
    let mut tmp = vec![0.0f32; src.len()];
    for _ in 0..iters {
        blur(&est, &mut conv, &mut tmp, w, h);
        for i in 0..src.len() {
            ratio[i] = src[i] / (conv[i] + 1e-3);
        }
        blur(&ratio, &mut corr, &mut tmp, w, h);
        for i in 0..src.len() {
            est[i] = (est[i] * corr[i]).clamp(0.0, 1.5);
        }
    }
    est
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conserva_una_imagen_plana() {
        let v = vec![0.4f32; 64 * 32];
        let out = richardson_lucy(&v, 64, 32, 4);
        for x in out {
            assert!((x - 0.4).abs() < 1e-3, "{}", x);
        }
    }

    #[test]
    fn sin_iteraciones_devuelve_la_entrada() {
        let v: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        let out = richardson_lucy(&v, 10, 10, 0);
        assert_eq!(out, v);
    }
}
