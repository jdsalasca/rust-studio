# Manifiesto

## El problema no es solo el modelo

Cuando una foto se ve mal después de pasar por una IA, la conversación suele
culpar al modelo. "Gemini cambió la cara", "la IA no entiende". El modelo tiene su
parte. Pero hay una variable anterior, que casi nadie mira y que sí podés
controlar: **el input**.

Una foto de móvil en una habitación oscura llega con la cara subexpuesta, el
color lavado, el foco blando y la cara ocupando un tercio del encuadre. El modelo
la reescala, la comprime en un latente y la reconstruye. Si la señal de identidad
es débil, rellena con su promedio de caras. No es maldad del modelo: es la
consecuencia de pedirle que adivine.

## La tesis

**La mayor brecha en generación y edición de imágenes no está en el modelo: está
en el input.** Un modelo bueno con un input pobre produce un resultado genérico.
Un modelo bueno con un input limpio, bien expuesto, enfocado, con la cara grande
y una referencia de identidad, tiene mucho menos que adivinar.

Esto no significa que el modelo dé igual. Significa que el input es la palanca
que más podés mover desde afuera, y la que más se subestima.

## Qué es "mejorar" una imagen

No es subir el contraste. Es un vector:

- **Foco**: recuperar el detalle óptico que existe pero está blando.
- **Ruido**: limpiar sin plastificar la textura.
- **Tono**: exponer sin quemar altas luces ni aplastar sombras.
- **Color**: balance de blancos y saturación natural, sin virajes.
- **Identidad**: que la cara llegue grande, nítida y con textura.
- **Artefactos**: sin halos, sin banding, sin grano de color.

El contraste global es solo la dimensión más fácil de ver en una miniatura.

## El modo IA

La IA no filtra la foto: la reconstruye. Por eso el modo IA del pipeline no
intenta "hacerla más linda". Prepara dos archivos:

- `_studio.jpg`: la foto completa, con contexto, para la edición.
- `_face.jpg`: la cara a 1024×1024, con la cara al ~75 % del cuadro, para que el
  modelo tenga más evidencia de identidad.

La cara pasa de ~30 % del ancho a ~75 %. En la foto completa, el modelo ve la
cara a ~540 px; en el recorte, a ~760 px cuando reescala a 1024. No es magia: es
darle al modelo menos fondo y más cara.

## La lógica del pipeline

1. **Medir antes de tocar**: luminancia, percentiles, mediana central,
   saturación, nitidez y ruido.
2. **Corregir lo global**: balance de blancos conservador y curva tonal.
3. **Limpiar**: bilateral suave y deconvolución Richardson–Lucy para recuperar
   foco sin inventar.
4. **Separar base y detalle**: filtro guiado; el detalle se amplifica con umbral
   para no levantar grano.
5. **Contraste local**: CLAHE sobre la base, no sobre la imagen completa, para no
   amplificar ruido.
6. **Limpiar color**: denoise cromático guiado por luminancia.
7. **Escalar y afilar**: Lanczos3 y unsharp con umbral.
8. **Recortar la cara**: detector SeetaFace y salida 1024×1024.

Cada paso tiene una razón medible. Nada se agrega "porque queda lindo".

## Determinista por diseño

Este proyecto no usa GAN, difusión ni *face restoration*. No porque no existan,
sino porque son exactamente lo que cambia la cara. La SR generativa puede dar un
salto visual más grande, pero inventa textura; la restauración facial puede
"arreglar" la piel, pero reemplaza rasgos. Acá la identidad es un requisito, no
un efecto.

Si algún día se agrega SR, será opcional, PSNR-oriented y con una advertencia
clara. El modo por defecto no negocia identidad.

## Lo que este proyecto no hace

- No recupera detalle que la cámara no capturó.
- No garantiza que el modelo de IA mantenga la cara.
- No reemplaza una máscara, una imagen de referencia ni una buena instrucción.
- No convierte una foto mala en una foto de estudio.

## Principios

1. **Medir antes de corregir.** Sin números, "mejorar" es una opinión.
2. **Limpiar, no inventar.** El detalle que no está no se dibuja.
3. **La cara es la identidad.** Si hay que elegir, se protege la cara.
4. **Una edición por pasada.** Cada regeneración acumula error.
5. **El original no se toca.** Todo se escribe en una carpeta nueva.
6. **Documentar el límite.** Una herramienta honesta dice qué no puede hacer.

## El flujo con IA

1. Subí `_face.jpg` como referencia de identidad.
2. Subí `_studio.jpg` para la edición.
3. Enmascará a la persona si el cambio es de fondo.
4. Una sola edición, con la fuerza más baja posible.
5. Compará contra `_face.jpg` antes de aceptar.

El prompt base y el detalle están en [`prompts/`](prompts/). La evidencia sobre
el drift de identidad está en [`docs/ai-fidelity.md`](docs/ai-fidelity.md).

---

Este manifiesto es una hipótesis de trabajo, no un dogma. Si un paso no mejora
una métrica o no reduce el drift, se saca.
