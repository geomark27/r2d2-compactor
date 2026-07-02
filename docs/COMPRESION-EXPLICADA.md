# Cómo funciona la compresión (explicado)

Documento de aprendizaje sobre **cómo R2D2 Compactor reduce el peso de videos e imágenes**, y sobre los conceptos de compresión y optimización que hay detrás. Está escrito para entender *el porqué*, no solo *el cómo*, y va conectando cada idea con el punto del código donde vive.

> Requisito previo: cero. Vamos de lo intuitivo a lo técnico.

---

## 1. El error de intuición (y el modelo mental correcto)

Lo natural es pensar: *"tomo el archivo pesado y lo aprieto hasta que pese menos"*, como quien comprime un `.zip`. Con video **eso no aplica**, y entender por qué es la llave de todo.

La relación fundamental de un video es:

```
peso (bits)  ≈  bitrate (bits por segundo)  ×  duración (segundos)
```

- **bitrate** = cuántos datos se gastan por *cada segundo* de video.
- **duración** = cuántos segundos dura.

O sea: el peso **no es una propiedad suelta** que aprieto; es la *consecuencia* de dos cosas. Y la que puedo controlar directamente al recodificar es el **bitrate**. El peso original (300 MB, 1 GB, lo que sea) es irrelevante para el cálculo — lo que importa es la duración.

Por eso, para llegar a un tamaño objetivo, despejo la fórmula:

```
bitrate_objetivo  =  tamaño_deseado  ÷  duración
```

**Esta es la razón por la que la app lee la duración del video (con `ffprobe`) y no su peso.**

📍 En el código: `ffmpeg.rs → probe_duration()` obtiene la duración; `queue.rs → compress_video()` hace el despeje.

---

## 2. El bitrate: la palanca central

Todo el arte de la compresión de video gira alrededor del bitrate:

- **Bitrate alto** → más datos por segundo → más detalle, más fluidez → archivo pesado.
- **Bitrate bajo** → menos datos por segundo → se ve más borroso o "cuadriculado" → archivo liviano.

Comprimir un video **es, en esencia, bajarle el bitrate** de forma inteligente para que se note lo menos posible.

En el proyecto, el cálculo real (para un objetivo en MB) es:

```
bits_totales   = tamaño_objetivo_MB × 8192 × 1024      (MB → kbits → bits)
bitrate_video  = (tamaño_objetivo_MB × 8192 ÷ duración) − 128
```

El `− 128` es porque una parte del presupuesto se reserva para el **audio** (`AUDIO_KBPS = 128`). El video se lleva el resto.

📍 En el código: constantes `AUDIO_KBPS` y `MIN_VIDEO_KBPS` en `queue.rs`.

### Ejemplo numérico (objetivo 5 MB)

| Duración del video | bitrate de video calculado | ¿Qué pasa? |
|---|---|---|
| 30 segundos | 40960 ÷ 30 − 128 ≈ **1237 kbps** | Alcanza los ~5 MB. Calidad menor, pero cumple. |
| 2.5 minutos | 40960 ÷ 150 − 128 ≈ **145 kbps** | Justo en el límite. |
| 10 minutos | 40960 ÷ 600 − 128 = **negativo** | Imposible: se activa el piso (ver §3). |

Fíjate que un video de **300 MB** puede quedar en **5 MB si es corto** (era de alto bitrate), pero no si es largo. El peso original nunca entró en la cuenta.

---

## 3. El "piso" de bitrate y por qué existe la advertencia

Si el objetivo es minúsculo y la duración enorme, la fórmula pide un bitrate tan bajo que el video sería un mosaico ilegible. Para evitar entregar basura, el proyecto pone un **piso**: `MIN_VIDEO_KBPS = 150`.

```
si bitrate_video < 150:
    bitrate_video = 150            # no bajamos de aquí
    advertencia = "no alcanzó el objetivo (video muy largo para ese tamaño)"
```

El resultado **superará** el tamaño pedido, pero será algo *usable* en vez de irreconocible. La app te avisa mostrando cuánto sí logró comprimir y por qué no llegó:

> ⚠ Listo · 93% más liviano (20.4 MB) — no alcanzó el objetivo (video muy largo para ese tamaño)

Esta es una decisión de **optimización con restricciones**: cuando dos objetivos chocan (tamaño vs. calidad mínima aceptable), se prioriza no producir un resultado inútil.

📍 En el código: el bloque `if video_kbps < MIN_VIDEO_KBPS` en `compress_video()`, y el armado del mensaje en `app.rs → poll()`.

---

## 4. Two-pass: por qué comprimimos en dos pasadas

En el código verás que cada video pasa por `-pass 1` y luego `-pass 2`. No es un capricho:

- **Pasada 1 (análisis):** FFmpeg recorre *todo* el video sin guardar nada (la salida va al "dispositivo nulo"). Va midiendo qué tramos son **complejos** (mucho movimiento, mucho detalle) y cuáles **simples** (una toma fija de una pared).
- **Pasada 2 (codificación real):** ahora que ya "conoce" el video, **reparte el presupuesto de bits de forma inteligente**: gasta más en las escenas difíciles y menos en las fáciles, manteniendo el promedio en el bitrate objetivo.

Comparación:

| | Una pasada | Two-pass |
|---|---|---|
| Reparto de bits | A ciegas, sobre la marcha | Informado, global |
| Precisión del tamaño final | Aproximada | Muy buena |
| Calidad al mismo peso | Menor | Mayor |
| Tiempo | 1× | ~2× (analiza y luego codifica) |

Por eso el progreso de la app se divide 50% / 50%: la primera mitad es el análisis, la segunda la codificación.

📍 En el código: `compress_video()` arma `args1` (pass 1, con `-an` para ignorar audio y salida a `null_device()`) y `args2` (pass 2, con audio AAC y `+faststart`).

---

## 5. Qué hace un códec por dentro (H.264 / libx264)

El **códec** es el algoritmo que realmente comprime. H.264 (el que usamos, vía `libx264`) exprime dos tipos de redundancia:

### a) Compresión temporal (entre cuadros)

Un video son muchas imágenes (cuadros) por segundo, y **cuadros seguidos se parecen muchísimo**. En vez de guardar cada cuadro completo, H.264 guarda tres tipos:

- **Cuadro I** (*intra* / keyframe): la imagen completa, independiente. Es el "punto de partida".
- **Cuadro P** (*predicho*): guarda solo **lo que cambió** respecto al cuadro anterior ("este bloque se movió 3 píxeles a la derecha").
- **Cuadro B** (*bidireccional*): se apoya en cuadros pasados **y futuros** para describirse con aún menos datos.

A la secuencia que va de un keyframe al siguiente se le llama **GOP** (*Group of Pictures*). Cuantos más cuadros P/B y menos I, más se comprime — pero se necesitan keyframes periódicos para poder "saltar" en la reproducción (por eso al arrastrar la barra de un video salta a keyframes).

> Esto explica por qué un video de una cámara fija comprime feroz (casi nada cambia entre cuadros) y uno lleno de movimiento/ruido comprime mal (cada cuadro es distinto). El video de prueba con "ruido" que se menciona en el README es justo el peor caso.

### b) Compresión espacial (dentro de un cuadro)

Dentro de una sola imagen también hay redundancia (zonas de color parecido). H.264 la trata parecido a un JPEG: divide el cuadro en bloques, les aplica una **transformada** (DCT) que separa lo importante (formas generales) de lo sutil (detalle fino), y luego **cuantiza**: redondea/descarta lo sutil.

### c) Compresión con pérdida

Ese "descartar lo sutil" es la **pérdida**: se tira información que el ojo humano casi no percibe (matices de color, detalle fino en movimiento rápido). Es irreversible, pero es lo que permite reducciones enormes. El **cuantizador** decide *cuánto* se tira: más agresivo = más liviano y más borroso.

### d) Paso final sin pérdida

Tras descartar, queda un flujo de datos que se empaqueta con **codificación de entropía** (CABAC/CAVLC), que es compresión *sin* pérdida (como un zip) — aprieta lo que ya quedó, sin tirar más.

---

## 6. El `-preset`: esfuerzo vs. tiempo

En el código el video usa `-preset medium`. El preset controla **cuánto se esfuerza el códec en buscar la mejor forma de comprimir**, no el tamaño objetivo:

- `slow` / `slower` → busca más → **mejor calidad al mismo peso**, pero tarda más.
- `fast` / `veryfast` → busca menos → más rápido, algo peor al mismo peso.
- `medium` → el equilibrio por defecto.

Clave: cambiar el preset **no cambia** el tamaño final (ese lo fija el bitrate); cambia la *calidad* que consigues para ese mismo tamaño, a cambio de tiempo de CPU.

📍 En el código: `"-preset", "medium"` en `compress_video()`. El README lo lista como "ajuste rápido".

---

## 7. Dos filosofías: bitrate objetivo vs. CRF

Hay dos formas de pedirle un resultado a un códec:

| | **Bitrate objetivo** (lo que usa esta app) | **CRF** (Constant Rate Factor) |
|---|---|---|
| Le dices... | "quiero *este tamaño*" | "quiero *esta calidad*" |
| Controlas | El peso final (predecible) | La calidad (constante) |
| El otro valor... | La calidad sale de lo que dé | El peso sale de lo que dé |
| Ideal para | Cumplir un límite (SharePoint 100 MB) | Archivar con calidad pareja |

Esta herramienta usa **bitrate objetivo con two-pass** porque el requisito real es *"que quepa bajo el límite de subida"* — un tamaño predecible importa más que una calidad constante. CRF sería la elección si el objetivo fuera "consérvalo bonito y que pese lo que tenga que pesar".

---

## 8. Imágenes: la misma meta, otra técnica

Una imagen **no tiene duración**, así que la fórmula del bitrate no aplica. Para apuntar al mismo "tamaño objetivo" con fotos, el proyecto usa otra estrategia: **búsqueda binaria de la calidad JPEG**.

- En JPEG la calidad se controla con un parámetro (`-q:v`) que en FFmpeg va de **2 (mejor) a 31 (peor)**. Cuanto peor la calidad, más liviano el archivo, **de forma monótona** (siempre en la misma dirección).
- Como es monótono, se puede hacer **búsqueda binaria**: probar una calidad, medir el peso, y según si pasó o no el objetivo, ajustar hacia mejor o peor calidad. En ~5 intentos encuentra la **mejor calidad que aún queda bajo el objetivo**.
- Si ni con la peor calidad se logra, se avisa y se sugiere bajar la resolución.

```
lo = 2 (mejor), hi = 31 (peor)
mientras lo <= hi:
    q = punto medio
    peso = codificar a calidad q
    si peso <= objetivo:  guardar q y buscar mejor calidad (bajar hi)
    si no:                buscar más compresión (subir lo)
```

Internamente el JPEG hace lo mismo que la parte *espacial* de H.264 (DCT + cuantización sobre bloques de 8×8), más **submuestreo de color** (`yuvj420p`: guarda el color a menos resolución que el brillo, porque el ojo es menos sensible al color).

📍 En el código: `queue.rs → compress_image()`, con `Worker::run_quiet()` para cada intento.

---

## 9. La resolución: la otra palanca

Además del bitrate/calidad, escalar reduce peso porque **hay menos píxeles que codificar**. Bajar de 1080p a 720p elimina más de la mitad de los píxeles por cuadro, liberando mucho presupuesto.

Regla del proyecto: el escalado **nunca agranda** (usa `min(altura, objetivo)`), solo reduce si la fuente es mayor. Así una foto ya pequeña no se infla artificialmente.

📍 En el código: `scale_filter()` en `queue.rs` → `scale=-2:min(ih\,{max_height})`. El `-2` mantiene la proporción y fuerza dimensiones pares (que los códecs necesitan). La coma va escapada (`\,`) porque FFmpeg la usa como separador.

---

## 10. Glosario rápido

- **Bitrate**: datos por segundo. La palanca principal del peso en video.
- **Códec**: algoritmo que comprime/descomprime (aquí H.264 vía libx264).
- **Keyframe / cuadro I**: imagen completa e independiente; punto de referencia.
- **GOP**: tramo entre dos keyframes.
- **DCT + cuantización**: separar lo importante de lo sutil y descartar lo sutil (donde ocurre la *pérdida*).
- **Compresión con pérdida**: tira información imperceptible; irreversible.
- **Preset**: cuánto se esfuerza el códec (calidad vs. tiempo), no cambia el tamaño.
- **CRF**: modo "calidad constante" (alternativa al bitrate objetivo).
- **Two-pass**: analizar primero para repartir bits mejor en la codificación.
- **Submuestreo de color (4:2:0)**: guardar el color a menos detalle que el brillo.
- **faststart**: mueve el índice del MP4 al inicio para que empiece a reproducirse antes en web.

---

## 11. Mapa: dónde vive cada cosa en el código

| Concepto | Archivo / función |
|---|---|
| Leer duración | `ffmpeg.rs → probe_duration` |
| Cálculo de bitrate y piso | `queue.rs → compress_video` (`AUDIO_KBPS`, `MIN_VIDEO_KBPS`) |
| Two-pass (args de pass 1 y 2) | `queue.rs → compress_video` |
| Preset / audio / faststart | `queue.rs → compress_video` |
| Búsqueda de calidad JPEG | `queue.rs → compress_image` |
| Escalado (resolución) | `queue.rs → scale_filter` |
| Ejecutar FFmpeg y leer progreso | `ffmpeg.rs → Worker::run_pass / run_quiet` |
| Mensaje de advertencia | `queue.rs` (texto) + `app.rs → poll` (armado) |

---

## 12. Para seguir aprendiendo

- **Documentación de FFmpeg**: <https://ffmpeg.org/documentation.html> — la referencia de todos los parámetros.
- **Guía de codificación H.264 de FFmpeg**: <https://trac.ffmpeg.org/wiki/Encode/H.264> — explica CRF, presets, two-pass en detalle.
- Conceptos para buscar y profundizar: *rate control*, *motion estimation*, *rate-distortion optimization*, *psychovisual optimization*, *chroma subsampling*, *entropy coding (CABAC)*.

Si quieres, podemos profundizar en cualquiera de estos temas o incluso experimentar cambiando parámetros en el código para *ver* el efecto en tamaño y calidad.
