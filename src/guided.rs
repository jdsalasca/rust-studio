//! Filtros edge-aware: media de caja, filtro guiado (He et al., 2010) y CLAHE.
//!
//! El filtro guiado separa una capa base suave del detalle sin invertir
//! gradientes (el problema clásico del bilateral). CLAHE se aplica solo a la
//! base, donde no amplifica ruido.

/// Media de caja con bordes replicados, en O(1) por píxel.
pub(crate) fn box_mean(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let k = (2 * r + 1) as f32;
    let mut tmp = vec![0.0f32; src.len()];
    let mut out = vec![0.0f32; src.len()];
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        let mut sum = row[0] * (r as f32 + 1.0);
        for i in 1..=r {
            sum += row[i.min(w - 1)];
        }
        for x in 0..w {
            tmp[y * w + x] = sum / k;
            sum += row[(x + r + 1).min(w - 1)] - row[x.saturating_sub(r)];
        }
    }
    for x in 0..w {
        let mut sum = tmp[x] * (r as f32 + 1.0);
        for i in 1..=r {
            sum += tmp[i.min(h - 1) * w + x];
        }
        for y in 0..h {
            out[y * w + x] = sum / k;
            sum += tmp[(y + r + 1).min(h - 1) * w + x] - tmp[y.saturating_sub(r) * w + x];
        }
    }
    out
}

/// Filtro guiado: `q ≈ a·I + b` por ventana, con `I = guide`.
pub(crate) fn guided_filter(
    guide: &[f32],
    input: &[f32],
    w: usize,
    h: usize,
    r: usize,
    eps: f32,
) -> Vec<f32> {
    let n = guide.len();
    let mi = box_mean(guide, w, h, r);
    let mp = box_mean(input, w, h, r);
    let mut scratch = vec![0.0f32; n];
    for i in 0..n {
        scratch[i] = guide[i] * guide[i];
    }
    let mut mii = box_mean(&scratch, w, h, r);
    for i in 0..n {
        scratch[i] = guide[i] * input[i];
    }
    let mut mip = box_mean(&scratch, w, h, r);
    drop(scratch);

    // a en mii, b en mip.
    for i in 0..n {
        let var = mii[i] - mi[i] * mi[i];
        let cov = mip[i] - mi[i] * mp[i];
        let a = cov / (var + eps);
        mip[i] = mp[i] - a * mi[i];
        mii[i] = a;
    }
    drop(mi);
    drop(mp);
    let ma = box_mean(&mii, w, h, r);
    let mb = box_mean(&mip, w, h, r);
    (0..n).map(|i| ma[i] * guide[i] + mb[i]).collect()
}

/// CLAHE sobre una capa 0..1. `blend = 0` devuelve la entrada.
///
/// `clip` limita el histograma (2.0 es suave, 4.0 agresivo). Las celdas casi
/// planas se dejan intactas para no inventar contraste donde no hay señal.
pub(crate) fn clahe(
    y: &[f32],
    w: usize,
    h: usize,
    tiles_x: usize,
    tiles_y: usize,
    clip: f32,
    blend: f32,
) -> Vec<f32> {
    if blend <= 0.0 {
        return y.to_vec();
    }
    let tx = tiles_x.max(1);
    let ty = tiles_y.max(1);
    let tw = w.div_ceil(tx);
    let th = h.div_ceil(ty);
    let mut luts = vec![[0u8; 256]; tx * ty];

    for tyi in 0..ty {
        for txi in 0..tx {
            let x0 = txi * tw;
            let y0 = tyi * th;
            if x0 >= w || y0 >= h {
                for (i, v) in luts[tyi * tx + txi].iter_mut().enumerate() {
                    *v = i as u8;
                }
                continue;
            }
            let x1 = (x0 + tw).min(w);
            let y1 = (y0 + th).min(h);
            let area = ((x1 - x0) * (y1 - y0)).max(1) as u32;
            let mut hist = [0u32; 256];
            let mut mn = 255usize;
            let mut mx = 0usize;
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let v = (y[yy * w + xx] * 255.0).round().clamp(0.0, 255.0) as usize;
                    hist[v] += 1;
                    mn = mn.min(v);
                    mx = mx.max(v);
                }
            }
            if mx.saturating_sub(mn) < 4 {
                for (i, v) in luts[tyi * tx + txi].iter_mut().enumerate() {
                    *v = i as u8;
                }
                continue;
            }
            let limit = ((clip * area as f32 / 256.0) as u32).max(1);
            let mut excess = 0u32;
            for n in &mut hist {
                if *n > limit {
                    excess += *n - limit;
                    *n = limit;
                }
            }
            let add = excess / 256;
            let rem = excess % 256;
            for (i, n) in hist.iter_mut().enumerate() {
                *n += add + u32::from(i < rem as usize);
            }
            let mut cdf = 0u32;
            let mut lut = [0u8; 256];
            for (i, slot) in lut.iter_mut().enumerate() {
                cdf += hist[i];
                *slot = ((cdf as f32 / area as f32) * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            luts[tyi * tx + txi] = lut;
        }
    }

    let mut out = vec![0.0f32; y.len()];
    for yy in 0..h {
        let gy = ((yy as f32 + 0.5) / th as f32 - 0.5).max(0.0);
        let y0 = gy.floor() as usize;
        let fy = (gy - y0 as f32).clamp(0.0, 1.0);
        let y1 = (y0 + 1).min(ty - 1);
        for xx in 0..w {
            let gx = ((xx as f32 + 0.5) / tw as f32 - 0.5).max(0.0);
            let x0 = gx.floor() as usize;
            let fx = (gx - x0 as f32).clamp(0.0, 1.0);
            let x1 = (x0 + 1).min(tx - 1);
            let v = (y[yy * w + xx] * 255.0).round().clamp(0.0, 255.0) as usize;
            let l00 = luts[y0 * tx + x0][v] as f32;
            let l10 = luts[y0 * tx + x1][v] as f32;
            let l01 = luts[y1 * tx + x0][v] as f32;
            let l11 = luts[y1 * tx + x1][v] as f32;
            let top = l00 + (l10 - l00) * fx;
            let bot = l01 + (l11 - l01) * fx;
            let c = (top + (bot - top) * fy) / 255.0;
            out[yy * w + xx] = y[yy * w + xx] + (c - y[yy * w + xx]) * blend;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caja_conserva_constante() {
        let v = vec![0.4f32; 64 * 32];
        let out = box_mean(&v, 64, 32, 7);
        for x in out {
            assert!((x - 0.4).abs() < 1e-5);
        }
    }

    #[test]
    fn guiado_conserva_constante() {
        let v = vec![0.3f32; 64 * 32];
        let out = guided_filter(&v, &v, 64, 32, 4, 0.01);
        for x in out {
            assert!((x - 0.3).abs() < 1e-4);
        }
    }

    #[test]
    fn clahe_plano_es_identidad() {
        let v = vec![0.5f32; 64 * 64];
        let out = clahe(&v, 64, 64, 8, 8, 2.0, 0.5);
        for x in out {
            assert!((x - 0.5).abs() < 2e-3);
        }
    }

    #[test]
    fn clahe_no_sale_de_rango() {
        let w = 65;
        let h = 33;
        let v: Vec<f32> = (0..w * h).map(|i| (i % 251) as f32 / 251.0).collect();
        let out = clahe(&v, w, h, 8, 8, 2.0, 0.6);
        for x in out {
            assert!((0.0..=1.0).contains(&x));
        }
    }
}
