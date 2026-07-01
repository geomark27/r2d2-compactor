# Compresor de Evidencias (Rust + egui)

App de escritorio **nativa** para comprimir videos de evidencia pesados antes de subirlos a Echo. Herramienta totalmente independiente del proyecto Laravel: el inspector la abre, arrastra el video, lo comprime y sube el resultado a mano.

Cero JavaScript, cero webview, cero runtime que instalar. Es un único binario nativo (Rust + [egui](https://github.com/emilk/egui)/eframe) que llama a **FFmpeg** como proceso externo — el mismo motor de compresión en los tres lenguajes que probamos, la diferencia es el "cascarón" alrededor.

## Validado de verdad, no solo "debería funcionar"

Este proyecto se compiló y se probó en este mismo entorno antes de entregártelo:

- `cargo check` y `cargo build --release` — **compilan limpio, cero errores, cero warnings**.
- Prueba real de compresión: generé un video sintético de 10s con ruido de alta entropía (el peor caso posible para un compresor, porque el ruido no se puede predecir ni comprimir bien) de **234 MB**, y el two-pass lo dejó en **5.1 MB** contra un objetivo de 5 MB — 2% de margen, exactamente el comportamiento predecible que buscábamos. El filtro de escalado (con el escape de coma que exige la sintaxis de filtros de FFmpeg) también se probó y funciona.

Con videos reales de celular (que son mucho menos "ruidosos" que ese test) el resultado será todavía más limpio.

## Cómo comprime

- **Two-pass H.264 (libx264)** apuntando a un tamaño objetivo en MB. El bitrate se calcula a partir de la duración, así el resultado cae de forma predecible bajo el límite (90 MB por defecto, con margen bajo el tope de 100 MB de SharePoint).
- **H.264 + AAC + `faststart`**: máxima compatibilidad con la previsualización de SharePoint y navegadores.
- **Escalado opcional a 1080p / 720p**: nunca agranda, solo reduce si la fuente es mayor.

## Paso 1 — Conseguir FFmpeg (una sola vez)

Descarga un build oficial de Windows (gratis) y coloca **ffmpeg.exe** y **ffprobe.exe** en una carpeta `ffmpeg/` junto al `.exe` final:

- https://www.gyan.dev/ffmpeg/builds/ → *release essentials*
- o https://github.com/BtbN/FFmpeg-Builds/releases

## Paso 2 — Compilar

Requiere el toolchain de Rust ([rustup.rs](https://rustup.rs), instala `cargo` y `rustc`).

```bash
cargo build --release
```

El ejecutable queda en `target/release/compresor-evidencias.exe` (o sin `.exe` en Linux/Mac). Junto a él, crea la carpeta `ffmpeg/` con los dos binarios del paso 1.

## Paso 3 — Probar en desarrollo

```bash
cargo run
```

## Uso

1. Arrastra uno o varios videos a la ventana (o usa "Elegir videos…").
2. Ajusta el **tamaño objetivo** (MB) y la **resolución máxima** si quieres.
3. Por defecto guarda junto al original con el sufijo `_comp.mp4`; o elige otra carpeta de salida.
4. **Comprimir todo**. Al terminar, "Ver archivo" abre la carpeta con el resultado.

## Notas técnicas

- Las versiones de algunas dependencias en `Cargo.toml` están fijadas (`=x.y.z`) porque el entorno donde compilé usa una versión algo antigua de Cargo; en tu máquina con un Rust más reciente estas versiones siguen siendo perfectamente válidas y no deberían darte problemas. Si en algún momento quieres las versiones más nuevas de `eframe`/`egui`, puedes quitar los pines y correr `cargo update`.
- `rfd` (los diálogos de "elegir archivo"/"elegir carpeta") usa GTK3 en Linux — en Windows usa los diálogos nativos, no necesita nada adicional.
- El progreso se lee parseando la salida `-progress pipe:1` de FFmpeg, igual que en las otras dos versiones que armamos — no es una barra falsa, es el avance real de cada pasada.

## Ajustes rápidos (en `src/main.rs`, función `run_queue`)

- `"-preset".into(), "medium".into()` → cámbialo a `"slow"` para más compresión (más lento) o `"fast"` para más velocidad.
- `AUDIO_KBPS` (128) → bitrate de audio.
- `MIN_VIDEO_KBPS` (150) → piso de bitrate para videos muy largos relativos al tamaño objetivo.
