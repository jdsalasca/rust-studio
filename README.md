# rust-studio

Pipeline de mejora de fotos de retrato escrito en Rust. Prepara las imágenes
para que un modelo generativo (Gemini, etc.) las edite sin cambiar la cara:
limpia tono, color, textura y tamaño, sin inventar rasgos.

El binario se llama `photo_studio`.

## Instalación

```powershell
cargo install --path . --force
```

Queda en `~\.cargo\bin` (normalmente ya está en el PATH).

## Uso

```powershell
# Toda una carpeta, salida en .\studio
photo_studio C:\fotos -o studio

# Solo diagnóstico, sin escribir nada
photo_studio foto.jpg --dry-run

# Conservar el tamaño original
photo_studio foto.jpg --long-side 0

# Ajustes más suaves
photo_studio foto.jpg --strength 0.7
```

| Opción | Por defecto | Qué hace |
| --- | --- | --- |
| `-o, --out` | `studio` | Carpeta de salida. |
| `--long-side` | `2560` | Lado largo de salida. Si la foto es más chica, escala con Lanczos; si es más grande, reduce. `0` deja el original. |
| `--strength` | `1.0` | Fuerza de vibrance, CLAHE, detalle y afilado (0 a 1.5). |
| `--dry-run` | — | Mide y muestra el diagnóstico, sin escribir. |

Salida: `{nombre}_studio.jpg`, JPEG calidad 95 con submuestreo 4:4:4.

## Pipeline

1. **Medición**: luminancia, percentiles, mediana central, saturación, nitidez y
   ruido de altas frecuencias.
2. **Balance de blancos**: gris-mundo parcial, corregido como máximo ±4 %.
3. **Curva tonal**: normaliza el rango y lleva la mediana de la zona central a
   ~0,46. Si la foto ya está bien expuesta, casi no la toca.
4. **Vibrance**: sube la saturación de las zonas apagadas sin quemar las que ya
   están saturadas.
5. **Denoise bilateral 3×3** sobre luminancia: suaviza zonas planas y respeta
   bordes.
6. **Base/detalle con filtro guiado** (He et al., 2010): separa la iluminación
   suave del detalle. El detalle se amplifica con una máscara de umbral para no
   levantar grano.
7. **CLAHE sobre la capa base** (Zuiderveld, 1994): contraste local en las zonas
   oscuras. Al aplicarse sobre la base suave, no amplifica ruido. Las celdas casi
   planas se dejan intactas.
8. **Denoise cromático guiado por luminancia**: suaviza el color sin tocar el
   detalle de luz, conservando anteojos, pelo y barba.
9. **Redimensionado Lanczos3** al lado largo pedido.
10. **Afilado final** suave con umbral, solo en luminancia.

## Base científica

- He, K., Sun, J., & Tang, X. (2010). *Guided Image Filtering*. ECCV 2010;
  TPAMI 35(6), 1397–1409 (2013). DOI 10.1109/TPAMI.2012.213.
  El filtro guiado preserva gradientes y evita los halos del bilateral.
- Zuiderveld, K. (1994). *Contrast Limited Adaptive Histogram Equalization*.
  Graphics Gems IV, 474–485. CLAHE con límite de recorte y celdas planas
  protegidas.
- Jobson, D. J., Rahman, Z., & Woodell, G. A. (1997). *Properties and
  performance of a center/surround retinex*. IEEE TIP 6(3), 451–462.
  La separación iluminación/reflectancia que inspira la capa base.
- Guo, J. (2025). *A Review of Traditional Image Enhancement Methods in Image
  Processing*. Computer Science and Application, 15(8), 250–259.
  DOI 10.12677/csa.2025.158215. Revisión 2025 de CLAHE, Retinex y gamma.

Los métodos deep learning (Retinexformer, Diff-Retinex++, GFPGAN y similares)
son el estado del arte en métricas, pero son generativos: reconstruyen la cara y
pueden cambiar la identidad. Este proyecto se queda en la familia determinista
a propósito.

## Para editar con Gemini

El modelo de imagen no filtra la foto: la vuelve a dibujar. Si la cara llega
oscura o blanda, rellena con su promedio de caras. Por eso:

1. Subí el `_studio.jpg`, no el original.
2. Pedí **una sola edición por pasada**.
3. Usá un prompt que fije la identidad:

   ```text
   Mantené exactamente la misma cara, identidad, rasgos, ojos, nariz, boca,
   barba y anteojos. No embellezcas, no suavices la piel ni cambies la forma
   de la cara. Solo ajustá [lo que querés cambiar].
   ```

4. Evitá "mejorá la calidad", "hacela más profesional" o "retocá la piel":
   eso es una invitación a regenerar la cara.
5. Si el cambio es de fondo, agregá: "no toques a la persona".

## Limitaciones

- No recupera detalle que la cámara no capturó; un upscale mayor que el original
  solo interpola. Para IA, 2560 de lado largo es suficiente.
- No corrige orientación EXIF (las fotos de móvil suelen venir derechas).
- No borra objetos, no cambia fondos ni reemplaza un RAW.
- El JPEG de salida pierde el EXIF original.
- El recorte automático de cara sigue pendiente: requiere un detector facial y,
  sin él, cualquier recorte sería una adivinanza.

## Calidad

```powershell
cargo test
cargo fmt --check
cargo clippy --bins -- -D warnings
```

El proyecto también pasa el gate portable del repo `teachers`:

```powershell
python tools/lint.py --project <ruta-a-este-repo>
```

## Licencia

Sin licencia definida. Si lo vas a reutilizar fuera de este repo, agregá una
(MIT es la más simple).
