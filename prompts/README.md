# Flujo recomendado para editar con IA sin perder la cara

1. **Subí `_face.jpg`** si la herramienta acepta una imagen de referencia de
   identidad. Es la cara a 1024×1024, con más píxeles que en la foto completa.
2. **Subí `_studio.jpg`** para la edición.
3. **Una sola edición por pasada.** No encadenes cinco retoques.
4. **Enmascará a la persona** si vas a cambiar el fondo. La máscara es más fuerte
   que cualquier prompt.
5. **Usá la fuerza más baja** que logre el cambio.
6. **Compará contra `_face.jpg`** antes de aceptar: ojos, nariz, boca, barba,
   anteojos y forma de la cara.

Prompt base: `prompts/gemini.txt`.

Evitá "mejorá la calidad", "hacela más profesional" o "retocá la piel": son
instrucciones abiertas que activan el prior de belleza del modelo.

El porqué está en `docs/ai-fidelity.md`.
