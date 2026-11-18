# rust-studio

Pipeline determinista de mejora de fotos de retrato, escrito en Rust. Prepara
las imágenes para que un modelo generativo (Gemini, etc.) las edite sin cambiar
la cara: corrige tono, color y textura, recupera foco y genera un recorte de cara
con más píxeles de identidad.

El binario se llama `photo_studio`. No usa GAN, difusión ni *face restoration*:
no inventa rasgos.

> El porqué del proyecto, en una página: [`MANIFESTO.md`](MANIFESTO.md).

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

# Conservar el tamaño original de la foto completa
photo_studio foto.jpg --long-side 0

# Ajustes más suaves o más deconvolución
photo_studio foto.jpg --strength 0.7 --deconv 5
```

| Opción | Por defecto | Qué hace |
| --- | --- | --- |
| `-o, --out` | `studio` | Carpeta de salida. |
| `--long-side` | `2560` | Lado largo de la foto completa. Si la foto es más chica, escala con Lanczos; si es más grande, reduce. `0` deja el original. |
| `--strength` | `1.0` | Fuerza de vibrance, CLAHE, detalle y afilado (0 a 1.5). |
| `--deconv` | `3` | Iteraciones de Richardson–Lucy sobre luminancia. `0` desactiva. |
| `--no-face-crop` | — | No generar el recorte de cara. |
| `--dry-run` | — | Mide y muestra el diagnóstico, sin escribir. |

Salida por foto:

- `{nombre}_studio.jpg`: foto completa, JPEG q95 4:4:4.
- `{nombre}_face.jpg`: cara a 1024×1024, con margen, para usar como referencia de
  identidad o para editar la cara con más píxeles.

## Pipeline

1. **Medición**: luminancia, percentiles, mediana central, saturación, nitidez y
   ruido de altas frecuencias.
2. **Balance de blancos**: gris-mundo parcial, corregido como máximo ±4 %.
3. **Curva tonal**: normaliza el rango y lleva la mediana de la zona central a
   ~0,46. Si la foto ya está bien expuesta, casi no la toca.
4. **Vibrance**: sube la saturación de las zonas apagadas sin quemar las que ya
   están saturadas.
5. **Denoise bilateral 3×3** sobre luminancia.
6. **Deconvolución Richardson–Lucy** (3 iteraciones): recupera foco con una PSF
   gaussiana. Es más honesta que un unsharp mask, pero amplifica ruido; por eso
   va seguida de un segundo bilateral que limpia el grano sin borrar bordes.
7. **Base/detalle con filtro guiado** (He et al., 2010): separa la iluminación
   suave del detalle y lo amplifica con una máscara de umbral.
8. **CLAHE sobre la capa base** (Zuiderveld, 1994): contraste local en zonas
   oscuras sin amplificar ruido. Las celdas casi planas se dejan intactas.
9. **Denoise cromático guiado por luminancia**: limpia el color sin tocar el
   detalle de luz.
10. **Redimensionado Lanczos3** y **afilado final** suave con umbral.

## Qué es "calidad" acá

No es un solo número. El pipeline ataca seis dimensiones:

| Dimensión | Qué se mide | Qué hace el pipeline |
| --- | --- | --- |
| Foco | Laplaciano medio | Deconvolución + afilado con umbral. |
| Ruido | Altas frecuencias en zonas planas | Bilateral antes y después del RL; CLAHE solo en la base. |
| Tono | Percentiles y mediana central | Curva con punto negro/blanco y gamma adaptativa. |
| Color | Saturación y medias por canal | WB ±4 % y vibrance con techo. |
| Identidad | Tamaño de cara y recorte | `_face.jpg` a 1024 px; sin modelos generativos. |
| Artefactos | Halos, clipping, grano | Filtro guiado, umbrales y segunda pasada bilateral. |

El contraste global es solo una parte. Si la foto de entrada es blanda, ninguna
técnica determinista puede duplicar el detalle que la cámara no capturó; el
objetivo es que la cara llegue limpia, con textura y con suficientes píxeles.

## Fidelidad de la IA

El modelo de imagen no filtra la foto: la reconstruye en un latente comprimido.
Por eso la cara cambia. El resumen de la evidencia y las citas está en
[`docs/ai-fidelity.md`](docs/ai-fidelity.md).

Lo que más ayuda, en orden:

1. **Recorte de cara** `_face.jpg` como referencia de identidad: la cara pasa de
   ~30 % del ancho a ~75 %, y de ~540 px a ~760 px cuando el modelo reescala a
   1024.
2. **Máscara** si el cambio es de fondo: la máscara pesa más que el prompt.
3. **Una sola edición por pasada** y fuerza baja.
4. **Prompt con invariantes explícitos** (`prompts/gemini.txt`).

Flujo completo: [`prompts/README.md`](prompts/README.md).

## Base científica

- He, K., Sun, J., & Tang, X. (2010). *Guided Image Filtering*. ECCV;
  TPAMI 35(6), 1397–1409 (2013). DOI 10.1109/TPAMI.2012.213.
- Zuiderveld, K. (1994). *Contrast Limited Adaptive Histogram Equalization*.
  Graphics Gems IV, 474–485.
- Jobson, D. J., Rahman, Z., & Woodell, G. A. (1997). *Properties and
  performance of a center/surround retinex*. IEEE TIP 6(3), 451–462.
- Richardson, W. H. (1972). *Bayesian-Based Iterative Method of Image
  Restoration*. JOSA 62(1), 55–59.
- Wu, S., Kan, M., He, Z., Shan, S., & Chen, X. (2016). *Funnel-structured
  cascade for multi-view face detection with alignment awareness*. Neurocomputing.
- Guo, J. (2025). *A Review of Traditional Image Enhancement Methods in Image
  Processing*. Computer Science and Application, 15(8), 250–259.
  DOI 10.12677/csa.2025.158215.

Los métodos deep learning (Retinexformer, Diff-Retinex++, GFPGAN, Real-ESRGAN)
son el estado del arte en métricas, pero son generativos: reconstruyen la cara y
pueden cambiar la identidad. Este proyecto se queda en la familia determinista a
propósito.

## Limitaciones

- No recupera detalle que la cámara no capturó; la deconvolución recupera foco,
  no información nueva.
- No corrige orientación EXIF.
- No borra objetos, no cambia fondos ni reemplaza un RAW.
- El JPEG de salida pierde el EXIF original.
- El detector de caras puede fallar en perfiles extremos, caras tapadas o
  miniaturas muy chicas; en ese caso no genera `_face.jpg`.
- No controla el modelo de IA: la identidad final depende de él.

## Calidad

```powershell
cargo test
cargo fmt --check
cargo clippy --bins -- -D warnings
```

Y el gate portable del repo `teachers`:

```powershell
python tools/lint.py --project <ruta-a-este-repo>
```

## Licencias

- Código: sin licencia definida. Si lo vas a reutilizar, agregá una (MIT es la
  más simple).
- Detector de caras: `rustface` (BSD-2-Clause) y modelo SeetaFace incluido en
  `models/`, con su licencia en `models/LICENSE.seetaface`.
