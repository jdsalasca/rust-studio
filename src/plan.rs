//! Decide los ajustes a partir de las medidas de la foto.

use crate::stats::{Stats, luma};

const TONE_N: usize = 1024;

/// Ajustes calculados para una foto concreta.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Ganancias de balance de blancos por canal.
    pub wb: [f32; 3],
    /// Punto negro de la curva tonal.
    pub lo: f32,
    /// Punto blanco de la curva tonal.
    pub hi: f32,
    /// Exponente de la curva: menor que 1 levanta las medianas.
    pub gamma: f32,
    /// Contraste en S suave.
    pub s_curve: f32,
    /// Cuánto subir la saturación de las zonas apagadas.
    pub vibrance: f32,
    /// Afilado final.
    pub sharpen: f32,
    /// Iteraciones de Richardson-Lucy (0 = sin deconvolución).
    pub deconv_iters: u32,
    /// Radio del filtro guiado que separa iluminación de detalle.
    pub base_radius: usize,
    /// Regularización del filtro guiado.
    pub base_eps: f32,
    /// Límite de recorte del histograma en CLAHE.
    pub clahe_clip: f32,
    /// Mezcla de CLAHE sobre la capa base.
    pub clahe_blend: f32,
    /// Cuánto se amplifica el detalle tras separar la base.
    pub detail_gain: f32,
    /// Radio del denoise cromático guiado.
    pub chroma_radius: usize,
    /// Mezcla del denoise cromático.
    pub chroma_blend: f32,
}

/// Calcula los ajustes. `strength` va de 0 a 1,5 y `long_side` es el lado largo de salida.
pub fn plan(stats: &Stats, strength: f32, long_side: u32, deconv_iters: u32) -> Plan {
    let s = strength.clamp(0.0, 1.5);

    // Gris-mundo parcial: corrige como mucho ±4 % y no toca el brillo.
    let mean_all = (stats.mean_rgb[0] + stats.mean_rgb[1] + stats.mean_rgb[2]) / 3.0;
    let mut wb = [1.0f32; 3];
    for (c, g) in wb.iter_mut().enumerate() {
        let m = stats.mean_rgb[c].max(0.02);
        *g = (mean_all / m).powf(0.35).clamp(0.96, 1.04);
    }
    let gain_l = luma(wb[0], wb[1], wb[2]).max(0.9);
    for g in &mut wb {
        *g = (*g / gain_l).clamp(0.95, 1.05);
    }

    // Gamma para llevar la mediana de la zona central a ~0,46.
    let span = (stats.hi - stats.lo).max(0.05);
    let mt = ((stats.center_median - stats.lo) / span).clamp(0.02, 0.98);
    let gamma = (0.46f32.ln() / mt.ln()).clamp(0.8, 1.3);

    Plan {
        wb,
        lo: stats.lo,
        hi: stats.hi,
        gamma,
        s_curve: 0.10 * s,
        vibrance: 0.50 * s,
        sharpen: if deconv_iters > 0 { 0.10 * s } else { 0.22 * s },
        deconv_iters,
        base_radius: ((long_side / 40) as usize).max(8),
        base_eps: 0.01,
        clahe_clip: 2.0,
        clahe_blend: 0.35 * s,
        detail_gain: 0.35 * s,
        chroma_radius: 4,
        chroma_blend: 0.55 * s,
    }
}

pub(crate) fn tone_table(plan: &Plan) -> Vec<f32> {
    let span = plan.hi - plan.lo;
    (0..TONE_N)
        .map(|i| {
            let v = i as f32 / (TONE_N - 1) as f32;
            let t = if span < 0.02 {
                v
            } else {
                ((v - plan.lo) / span).clamp(0.0, 1.0)
            };
            let g = t.powf(plan.gamma);
            (g + plan.s_curve * g * (1.0 - g) * (2.0 * g - 1.0)).clamp(0.0, 1.0)
        })
        .collect()
}

pub(crate) fn tone_at(table: &[f32], v: f32) -> f32 {
    let i = (v * (TONE_N - 1) as f32).clamp(0.0, (TONE_N - 1) as f32) as usize;
    table[i]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_stats() -> Stats {
        Stats {
            mean: 0.33,
            lo: 0.06,
            hi: 0.95,
            center_median: 0.45,
            saturation: 0.14,
            sharpness: 0.007,
            noise: 0.5,
            mean_rgb: [0.325, 0.297, 0.307],
        }
    }

    #[test]
    fn gamma_es_razonable() {
        let p = plan(&sample_stats(), 1.0, 4608, 4);
        assert!(p.gamma >= 0.8 && p.gamma <= 1.3, "gamma {}", p.gamma);
    }

    #[test]
    fn wb_conservador() {
        let p = plan(&sample_stats(), 1.0, 4608, 4);
        for g in p.wb {
            assert!((0.95..=1.05).contains(&g), "ganancia {}", g);
        }
    }

    #[test]
    fn curva_tonal_monotona() {
        let p = plan(&sample_stats(), 1.0, 4608, 4);
        let t = tone_table(&p);
        for i in 1..t.len() {
            assert!(t[i] >= t[i - 1] - 1e-6, "baja en {}", i);
        }
        assert!(t[0] <= 1e-6);
        assert!((t[TONE_N - 1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn gamma_sube_medianas() {
        let mut s = sample_stats();
        s.center_median = 0.30;
        let p = plan(&s, 1.0, 4608, 4);
        let t = tone_table(&p);
        let v = tone_at(&t, 0.30);
        assert!(v > 0.33, "0.30 quedó en {}", v);
    }
}
