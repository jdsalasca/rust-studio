# Por qué la IA cambia la cara

Resumen de la evidencia que explica el drift de identidad en modelos de imagen, y
qué hace este pipeline al respecto. No es un paper: es una guía de decisiones.

## 1. La imagen no se filtra, se reconstruye

Los modelos de difusión modernos no aplican un filtro a la foto. La codifican en
un espacio latente comprimido, editan ahí y la decodifican. Ese cuello de botella
es lossy: los detalles finos de la cara (poros, lunares, forma exacta del
párpado) no viajan intactos.

- Rombach et al. (2022), *High-Resolution Image Synthesis with Latent Diffusion
  Models*, CVPR. El autoencoder perceptual comprime la imagen; el modelo genera
  en el latente y el decoder reconstruye.
- La mayoría de los modelos se entrenan con resoluciones o *buckets* fijos, así
  que la entrada se reescala. Una foto 9:16 de 2592×4608 termina con la cara a
  ~500 px aunque el archivo original tenga 1548 px de cara.

**Consecuencia:** mandar más resolución ayuda solo hasta el tamaño nativo del
modelo. Lo que sí ayuda es que la cara ocupe más proporción del encuadre.

## 2. La identidad se pierde cuando hay poca evidencia visual

El benchmark de identidad persistente (arXiv:2609.04151, 2026) mide similitud de
identidad contra la fuente limpia y encuentra que la brecha crece a medida que se
degrada la evidencia visual: cuanto menos detalle de cara llega, más se apoya el
modelo en su prior de "cara promedio".

**Consecuencia:** un recorte de cara con la cara grande y nítida es más valioso
que subir la foto completa a 8K. Por eso `photo_studio` genera `_face.jpg` a
1024×1024 además de la foto completa.

## 3. Sin condicionamiento explícito de identidad, el drift es esperable

Los métodos que mejor preservan identidad agregan una señal explícita: atención
cruzada a la referencia, *identity loss*, o anclaje de auto-atención.

- FISA (2026), *Identity and structure preserving face editing via diffusion
  models*, Multimedia Systems. DOI 10.1007/s00530-025-02183-9.
- IPNet (ICLR 2025), *InstantPortrait*: usa una *Constant Identity Loss*.
- DreamSalon (CVPR 2024): edición por etapas para preservar identidad y contexto.

Un prompt que dice "mantené la cara" no reemplaza esa señal; solo sesga.

**Consecuencia:** el prompt ayuda, pero no es una garantía. Si la herramienta que
usás permite imagen de referencia o máscara, eso pesa más que cualquier texto.

## 4. "Mejorar" sesga hacia embellecer

Los datasets de retrato están retocados. Cuando pedís "mejorá la calidad" o
"hacela más profesional", el modelo interpreta una instrucción abierta y aplica
su prior de belleza: piel lisa, mandíbula distinta, ojos más grandes.

**Consecuencia:** pedí una edición concreta ("cambiá el fondo a gris", "subí la
exposición") y agregá invariantes explícitos. Evitá instrucciones globales de
calidad.

## 5. Cada pasada acumula error

Cada edición vuelve a codificar y decodificar la imagen. Hacer cinco pasadas
sucesivas acumula pérdida de detalle y drift, aunque cada paso sea pequeño.

**Consecuencia:** una sola edición por pasada, y compará contra el recorte de
cara original antes de aceptar.

## 6. La superresolución generativa también cambia la cara

Los modelos tipo ESRGAN/Real-ESRGAN priorizan realismo perceptual con una
adversarial loss que no es pairwise: pueden generar una cara nítida pero distinta.
Los modelos PSNR-oriented (EDSR, SwinIR) preservan mejor la identidad, pero
inventan menos detalle.

- Zhang et al. (2018), *Super-Identity Convolutional Neural Network for Face
  Hallucination*, ECCV: la SR adversarial pierde identidad; una pérdida de
  identidad la recupera.
- Grm et al. (2018), *Face Hallucination Revisited: An Exploratory Study on
  Dataset Bias*: los modelos entrenados con degradación bicúbica fallan con
  degradaciones reales de cámara.

**Consecuencia:** este proyecto no usa SR generativa por defecto. Usa
deconvolución determinista (Richardson–Lucy) y un recorte de cara grande. Si
algún día se agrega SR, tiene que ser PSNR-oriented y opcional, nunca el modo
"máximo detalle" por defecto.

## 7. La fuerza de edición importa

En image-to-image, el parámetro de fuerza o *denoising strength* decide cuánto se
aleja del original. SDEdit (Meng et al., ICLR 2022) formaliza ese compromiso:
más fuerza = más realismo generado, menos fidelidad a la entrada.

**Consecuencia:** para retoques, usá la fuerza más baja que logre el cambio. Para
un cambio de fondo, enmascarar a la persona es más efectivo que pedirle al modelo
que "no la toque".

## Qué hace `rust-studio`

- **`_face.jpg` (1024×1024)**: recorte de la cara con margen, para subir como
  referencia o para editar la cara con más píxeles de identidad.
- **`_studio.jpg`**: foto completa con tono, color y textura corregidos, sin
  regenerar nada.
- **Deconvolución Richardson–Lucy**: recupera foco real, no inventa textura.
- **Sin modelos generativos**: no hay GAN, no hay difusión, no hay *face
  restoration*.
- **Prompt y flujo**: `prompts/gemini.txt` y la sección del README.

## Qué NO puede hacer

- No controla el modelo de Gemini ni su VAE.
- No garantiza identidad: eso depende del modelo, la máscara y la fuerza.
- No recupera detalle que la cámara no capturó.
- No reemplaza una imagen de referencia de identidad ni una máscara.

## Referencias

1. Rombach, R., Blattmann, A., Lorenz, D., Esser, P., & Ommer, B. (2022).
   *High-Resolution Image Synthesis with Latent Diffusion Models*. CVPR.
   arXiv:2112.10752.
2. Podell, D., English, Z., Lacey, K., et al. (2023). *SDXL: Improving Latent
   Diffusion Models for High-Resolution Image Synthesis*. arXiv:2307.01952.
3. Ren, M., Zhang, C., & Xia, Z. (2026). *Persistent Identity Preservation in
   Generative Image Models: A Benchmark and Evaluation System*. arXiv:2609.04151.
4. *Identity and structure preserving face editing via diffusion models* (2026).
   Multimedia Systems. DOI 10.1007/s00530-025-02183-9.
5. *InstantPortrait: One-Step Portrait Editing via Diffusion Multi-Objective
   Distillation* (2025). ICLR.
6. Lin, Y., et al. (2024). *DreamSalon: A Staged Diffusion Framework for
   Preserving Identity-Context in Editable Face Generation*. CVPR.
7. Meng, C., et al. (2022). *SDEdit: Guided Image Synthesis and Editing with
   Stochastic Differential Equations*. ICLR. arXiv:2108.01073.
8. Zhang, K., et al. (2018). *Super-Identity Convolutional Neural Network for
   Face Hallucination*. ECCV.
9. Grm, K., et al. (2018). *Face Hallucination Revisited: An Exploratory Study on
   Dataset Bias*. arXiv:1812.09010.
10. Brooks, T., Holynski, A., & Efros, A. A. (2023). *InstructPix2Pix: Learning
    to Follow Image Editing Instructions*. CVPR.
