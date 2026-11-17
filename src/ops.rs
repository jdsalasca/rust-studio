//! Operaciones de imagen: balance, tono, textura, redimensionado y afilado.

use image::RgbImage;

use crate::guided;
use crate::plan::{Plan, tone_at, tone_table};
use crate::stats::luma;

fn luma_vec(img: &RgbImage) -> Vec<f32> {
    let src = img.as_raw();
    (0..src.len() / 3)
        .map(|i| {
            luma(
                src[i * 3] as f32 / 255.0,
                src[i * 3 + 1] as f32 / 255.0,
                src[i * 3 + 2] as f32 / 255.0,
            )
        })
        .collect()
}

/// Aplica un factor de brillo por píxel conservando el color.
fn scale_by_luma(img: &mut RgbImage, old: &[f32], new: &[f32]) {
    let src = img.as_mut();
    for i in 0..old.len() {
        let y = old[i];
        if y > 1e-4 {
            let s = (new[i] / y).clamp(0.0, 4.0);
            for c in 0..3 {
                let v = src[i * 3 + c] as f32 * s;
                src[i * 3 + c] = v.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

fn gauss3(src: &[f32], w: usize, h: usize) -> Vec<f32> {
    let mut tmp = vec![0.0f32; src.len()];
    let mut out = vec![0.0f32; src.len()];
    for y in 0..h {
        for x in 0..w {
            let l = src[y * w + x.saturating_sub(1)];
            let c = src[y * w + x];
            let r = src[y * w + (x + 1).min(w - 1)];
            tmp[y * w + x] = (l + 2.0 * c + r) * 0.25;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let u = tmp[y.saturating_sub(1) * w + x];
            let c = tmp[y * w + x];
            let d = tmp[(y + 1).min(h - 1) * w + x];
            out[y * w + x] = (u + 2.0 * c + d) * 0.25;
        }
    }
    out
}

/// Bilateral 3x3 sobre la luminancia: suaviza zonas planas y respeta bordes.
fn bilateral(y: &[f32], w: usize, h: usize) -> Vec<f32> {
    let mut range = [0f32; 256];
    for (i, v) in range.iter_mut().enumerate() {
        let d = i as f32 / 255.0;
        *v = (-(d * d) / (2.0 * 0.03 * 0.03)).exp();
    }
    let spatial = [
        [0.600f32, 0.775, 0.600],
        [0.775, 1.0, 0.775],
        [0.600, 0.775, 0.600],
    ];
    let mut out = vec![0f32; y.len()];
    for yy in 0..h {
        for xx in 0..w {
            let c = y[yy * w + xx];
            let mut acc = 0f32;
            let mut wsum = 0f32;
            for (dy, weights) in spatial.iter().enumerate() {
                let ny = (yy + dy).saturating_sub(1).min(h - 1);
                for (dx, &weight) in weights.iter().enumerate() {
                    let nx = (xx + dx).saturating_sub(1).min(w - 1);
                    let v = y[ny * w + nx];
                    let d = ((v - c).abs() * 255.0).round().clamp(0.0, 255.0) as usize;
                    let wt = range[d] * weight;
                    acc += v * wt;
                    wsum += wt;
                }
            }
            out[yy * w + xx] = acc / wsum;
        }
    }
    out
}

/// Ejecuta el plan y devuelve la imagen mejorada, a resolución original.
pub fn apply(img: &RgbImage, plan: &Plan) -> Result<RgbImage, String> {
    let (w, h) = img.dimensions();
    let (wu, hu) = (w as usize, h as usize);
    let src = img.as_raw();

    // 1. Balance de blancos + curva tonal.
    let mut wb_lut = [[0u8; 256]; 3];
    for (c, lut) in wb_lut.iter_mut().enumerate() {
        for (v, slot) in lut.iter_mut().enumerate() {
            *slot = (v as f32 * plan.wb[c]).round().clamp(0.0, 255.0) as u8;
        }
    }
    let tone = tone_table(plan);
    let mut buf = vec![0u8; src.len()];
    for i in 0..src.len() / 3 {
        let r = wb_lut[0][src[i * 3] as usize] as f32 / 255.0;
        let g = wb_lut[1][src[i * 3 + 1] as usize] as f32 / 255.0;
        let b = wb_lut[2][src[i * 3 + 2] as usize] as f32 / 255.0;
        let y = luma(r, g, b);
        let y2 = tone_at(&tone, y);
        let s = if y > 1e-4 {
            (y2 / y).clamp(0.0, 4.0)
        } else {
            0.0
        };
        buf[i * 3] = (r * s * 255.0).round().clamp(0.0, 255.0) as u8;
        buf[i * 3 + 1] = (g * s * 255.0).round().clamp(0.0, 255.0) as u8;
        buf[i * 3 + 2] = (b * s * 255.0).round().clamp(0.0, 255.0) as u8;
    }

    // 2. Vibrance: sube más las zonas poco saturadas y protege las que ya lo están.
    for i in 0..buf.len() / 3 {
        let r = buf[i * 3] as f32 / 255.0;
        let g = buf[i * 3 + 1] as f32 / 255.0;
        let b = buf[i * 3 + 2] as f32 / 255.0;
        let mx = r.max(g).max(b);
        let mn = r.min(g).min(b);
        let sat = if mx > 1e-4 { (mx - mn) / mx } else { 0.0 };
        let gain = (1.0 + plan.vibrance * (1.0 - sat)).min(1.35);
        let l = luma(r, g, b);
        buf[i * 3] = ((l + (r - l) * gain) * 255.0).round().clamp(0.0, 255.0) as u8;
        buf[i * 3 + 1] = ((l + (g - l) * gain) * 255.0).round().clamp(0.0, 255.0) as u8;
        buf[i * 3 + 2] = ((l + (b - l) * gain) * 255.0).round().clamp(0.0, 255.0) as u8;
    }

    let mut out =
        RgbImage::from_raw(w, h, buf).ok_or_else(|| "búfer de imagen inválido".to_string())?;

    // 3. Denoise bilateral suave sobre luminancia, antes del microcontraste.
    let y0 = luma_vec(&out);
    let y1 = bilateral(&y0, wu, hu);
    scale_by_luma(&mut out, &y0, &y1);

    // 4. Base/detalle con filtro guiado + CLAHE en la base (sin amplificar ruido).
    if plan.clahe_blend > 0.0 || plan.detail_gain > 0.0 {
        let y2 = luma_vec(&out);
        let base = guided::guided_filter(&y2, &y2, wu, hu, plan.base_radius, plan.base_eps);
        let base2 = guided::clahe(&base, wu, hu, 8, 8, plan.clahe_clip, plan.clahe_blend);
        let mut y3 = Vec::with_capacity(y2.len());
        for i in 0..y2.len() {
            let d = y2[i] - base[i];
            let t = ((d.abs() - 2.0 / 255.0) / (6.0 / 255.0)).clamp(0.0, 1.0);
            let mask = t * t * (3.0 - 2.0 * t);
            y3.push((base2[i] + d * (1.0 + plan.detail_gain * mask)).clamp(0.0, 1.0));
        }
        scale_by_luma(&mut out, &y2, &y3);
    }

    // 5. Denoise cromático guiado por la luminancia.
    denoise_chroma(&mut out, plan.chroma_radius, plan.chroma_blend);

    Ok(out)
}

/// Suaviza el color sin tocar la luminancia. El guía es Y, así que los bordes de
/// luz (anteojos, pelo) se conservan aunque el croma se suavice.
fn denoise_chroma(img: &mut RgbImage, radius: usize, blend: f32) {
    if blend <= 0.0 {
        return;
    }
    let (w, h) = img.dimensions();
    let (wu, hu) = (w as usize, h as usize);
    let n = wu * hu;
    let mut y = vec![0.0f32; n];
    let mut cb = vec![0.0f32; n];
    let mut cr = vec![0.0f32; n];
    {
        let src = img.as_raw();
        for i in 0..n {
            let r = src[i * 3] as f32 / 255.0;
            let g = src[i * 3 + 1] as f32 / 255.0;
            let b = src[i * 3 + 2] as f32 / 255.0;
            let yy = luma(r, g, b);
            y[i] = yy;
            cb[i] = b - yy;
            cr[i] = r - yy;
        }
    }
    let cb2 = guided::guided_filter(&y, &cb, wu, hu, radius, 0.005);
    let cr2 = guided::guided_filter(&y, &cr, wu, hu, radius, 0.005);
    let out = img.as_mut();
    for i in 0..n {
        let yy = y[i];
        let cbv = cb[i] + (cb2[i] - cb[i]) * blend;
        let crv = cr[i] + (cr2[i] - cr[i]) * blend;
        let r = yy + crv;
        let b = yy + cbv;
        let g = (yy - 0.2126 * r - 0.0722 * b) / 0.7152;
        out[i * 3] = (r * 255.0).round().clamp(0.0, 255.0) as u8;
        out[i * 3 + 1] = (g * 255.0).round().clamp(0.0, 255.0) as u8;
        out[i * 3 + 2] = (b * 255.0).round().clamp(0.0, 255.0) as u8;
    }
}

/// Redimensiona al lado largo indicado con Lanczos3. `target == 0` no cambia nada.
pub fn resize_long_side(img: &RgbImage, target: u32) -> RgbImage {
    let (w, h) = img.dimensions();
    if target == 0 {
        return img.clone();
    }
    let long = w.max(h);
    if long == target {
        return img.clone();
    }
    let scale = target as f32 / long as f32;
    let nw = ((w as f32 * scale).round() as u32).max(1);
    let nh = ((h as f32 * scale).round() as u32).max(1);
    image::imageops::resize(img, nw, nh, image::imageops::FilterType::Lanczos3)
}

/// Afilado final suave, después de redimensionar.
pub fn sharpen(img: &mut RgbImage, amount: f32) {
    if amount <= 0.0 {
        return;
    }
    let (w, h) = img.dimensions();
    let (wu, hu) = (w as usize, h as usize);
    let y = luma_vec(img);
    let blur = gauss3(&y, wu, hu);
    let th = 6.0 / 255.0;
    let mut y2 = y.clone();
    for i in 0..y.len() {
        let d = y[i] - blur[i];
        let ds = if d > th {
            d - th
        } else if d < -th {
            d + th
        } else {
            0.0
        };
        y2[i] = (y[i] + amount * ds).clamp(0.0, 1.0);
    }
    scale_by_luma(img, &y, &y2);
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn redimensiona_al_lado_largo() {
        let img = RgbImage::new(2592, 4608);
        let out = resize_long_side(&img, 2560);
        assert_eq!(out.dimensions(), (1440, 2560));
        let same = resize_long_side(&img, 0);
        assert_eq!(same.dimensions(), (2592, 4608));
    }

    #[test]
    fn apply_conserva_tamano() {
        let img = RgbImage::from_pixel(64, 64, Rgb([80, 70, 60]));
        let p = Plan {
            wb: [1.0; 3],
            lo: 0.1,
            hi: 0.8,
            gamma: 0.9,
            s_curve: 0.1,
            vibrance: 0.3,
            sharpen: 0.2,
            base_radius: 6,
            base_eps: 0.01,
            clahe_clip: 2.0,
            clahe_blend: 0.3,
            detail_gain: 0.6,
            chroma_radius: 4,
            chroma_blend: 0.5,
        };
        let out = apply(&img, &p).unwrap();
        assert_eq!(out.dimensions(), (64, 64));
    }
}
