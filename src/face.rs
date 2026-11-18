//! Detección y recorte de cara con SeetaFace (modelo incluido, licencia BSD-2).

use std::io::Cursor;

use image::{RgbImage, imageops};

const MODEL: &[u8] = include_bytes!("../models/seeta_fd_frontal_v1.0.bin");

/// Caja de la cara más grande detectada, en coordenadas de la imagen original.
#[derive(Debug, Clone, Copy)]
pub struct FaceBox {
    /// Coordenada X de la esquina superior izquierda.
    pub x: u32,
    /// Coordenada Y de la esquina superior izquierda.
    pub y: u32,
    /// Ancho de la caja.
    pub w: u32,
    /// Alto de la caja.
    pub h: u32,
}

/// Detecta la cara más grande. Devuelve `None` si no hay ninguna.
pub fn detect(img: &RgbImage) -> Option<FaceBox> {
    let (w, h) = img.dimensions();
    if w < 48 || h < 48 {
        return None;
    }
    // El detector trabaja sobre una copia de 640 px de lado largo: suficiente
    // para una cara de retrato y mucho más rápido que a resolución completa.
    let scale = (640.0 / w.max(h) as f32).min(1.0);
    let dw = ((w as f32 * scale).round() as u32).max(1);
    let dh = ((h as f32 * scale).round() as u32).max(1);
    let small = imageops::resize(img, dw, dh, imageops::FilterType::Triangle);
    let gray = image::DynamicImage::ImageRgb8(small).to_luma8();

    let model = rustface::read_model(Cursor::new(MODEL)).ok()?;
    let mut detector = rustface::create_detector_with_model(model);
    detector.set_min_face_size(32);
    detector.set_slide_window_step(4, 4);
    let data = rustface::ImageData::new(gray.as_raw(), dw, dh);
    let faces = detector.detect(&data);

    let best = faces
        .iter()
        .filter(|f| f.score() > 0.5)
        .max_by_key(|f| f.bbox().width() * f.bbox().height())?;

    let sx = w as f32 / dw as f32;
    let sy = h as f32 / dh as f32;
    let x = (best.bbox().x().max(0) as f32 * sx).round() as u32;
    let y = (best.bbox().y().max(0) as f32 * sy).round() as u32;
    let fw = (best.bbox().width() as f32 * sx).round() as u32;
    let fh = (best.bbox().height() as f32 * sy).round() as u32;
    Some(FaceBox {
        x: x.min(w - 1),
        y: y.min(h - 1),
        w: fw.min(w - x),
        h: fh.min(h - y),
    })
}

/// Recorta un cuadrado alrededor de la cara, con margen, y lo escala a `size`.
pub fn crop(img: &RgbImage, face: &FaceBox, size: u32) -> RgbImage {
    let (w, h) = img.dimensions();
    let side = (face.w.max(face.h) as f32 * 1.35)
        .max(64.0)
        .min(w as f32)
        .min(h as f32);
    let half = side / 2.0;
    let cx = (face.x as f32 + face.w as f32 / 2.0).clamp(half, w as f32 - half);
    let cy = (face.y as f32 + face.h as f32 / 2.0).clamp(half, h as f32 - half);
    let s = side.round() as u32;
    let x0 = (cx - half).round() as u32;
    let y0 = (cy - half).round() as u32;
    let x0 = x0.min(w.saturating_sub(s));
    let y0 = y0.min(h.saturating_sub(s));
    let cut = imageops::crop_imm(img, x0, y0, s, s).to_image();
    imageops::resize(&cut, size, size, imageops::FilterType::Lanczos3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn imagen_plana_no_tiene_caras() {
        let img = RgbImage::from_pixel(128, 128, Rgb([120, 120, 120]));
        assert!(detect(&img).is_none());
    }

    #[test]
    fn recorte_respeta_el_tamano() {
        let img = RgbImage::from_pixel(400, 600, Rgb([80, 80, 80]));
        let face = FaceBox {
            x: 100,
            y: 150,
            w: 200,
            h: 200,
        };
        let cut = crop(&img, &face, 256);
        assert_eq!(cut.dimensions(), (256, 256));
    }
}
