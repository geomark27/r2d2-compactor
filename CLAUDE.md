# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Qué es

App de escritorio nativa (Rust + egui/eframe) para comprimir videos de evidencia pesados a un tamaño objetivo en MB antes de subirlos a SharePoint/Echo. No es un servicio ni tiene backend: es un único binario que orquesta **FFmpeg como proceso externo**.

## Módulos (`src/`)

| Archivo | Responsabilidad |
|---------|-----------------|
| `main.rs` | Punto de entrada: declara los módulos y arranca la ventana eframe. |
| `model.rs` | Tipos de dominio: `Msg` (canal trabajo→UI), `Job`, `JobState`. |
| `util.rs` | Helpers puros sin dependencias del resto: `fmt_size`, `parse_out_time`, `open_containing_folder`. |
| `ffmpeg.rs` | Localización e invocación de FFmpeg/FFprobe: `resolve_tool`, `which_in_path`, `null_device`, `probe_duration`, y el struct `Worker` con `Worker::run_pass` (contexto compartido del hilo: ruta de ffmpeg, canal `tx`, `cancel_flag`, `current_child`). |
| `queue.rs` | Lógica de compresión two-pass en el hilo de trabajo: `run_queue`, constantes `AUDIO_KBPS`/`MIN_VIDEO_KBPS`, `cleanup_passlog`. |
| `app.rs` | GUI egui: struct `App` (estado) + `impl eframe::App` (renderizado y polling). |

Dependencias entre módulos (sin ciclos): `app` → {`ffmpeg`, `queue`, `model`, `util`}; `queue` → {`ffmpeg`, `model`}; `ffmpeg` → {`model`, `util`}.

## Comandos

```bash
cargo build --release      # binario en target/release/compresor-evidencias(.exe)
cargo run                  # ejecutar en desarrollo
cargo check                # verificación rápida sin compilar el binario
```

No hay tests ni linter configurados en el repo.

## Dependencia de FFmpeg (crítico)

La app **no** enlaza FFmpeg; lo invoca como binario externo vía `resolve_tool()`:
1. Busca `ffmpeg`/`ffprobe` (`.exe` en Windows) en una carpeta `ffmpeg/` **junto al ejecutable**.
2. Si no está, hace fallback al `PATH` del sistema (útil en desarrollo con FFmpeg instalado).

Para distribuir hay que copiar la carpeta `ffmpeg/` con los dos binarios junto al `.exe`. La carpeta `ffmpeg/` del repo solo contiene una nota (`COLOCAR_FFMPEG_AQUI.txt`), no los binarios. Si FFmpeg falta, la UI lo detecta al arranque (`ffmpeg_missing`) y bloquea la compresión.

## Arquitectura

Modelo de dos hilos con comunicación por canal, típico de una GUI que lanza trabajo pesado:

- **Hilo de UI** (`impl eframe::App for App`, `update()`): dibuja todo cada frame en modo inmediato. Mantiene la lista `jobs: Vec<Job>` como fuente de verdad del estado visible. Cada frame llama `poll()` para drenar mensajes.
- **Hilo de trabajo** (`run_queue`, lanzado en `start_run()`): procesa la cola de videos secuencialmente y ejecuta FFmpeg. Se comunica con la UI **solo** mediante `Sender<Msg>` (variantes `Progress`/`Done`/`Error`/`Canceled`).

Estado compartido entre hilos:
- `cancel_flag: Arc<AtomicBool>` — la UI lo marca al pulsar "Cancelar"; el hilo de trabajo lo consulta en cada línea de progreso.
- `current_child: Arc<Mutex<Option<Child>>>` — referencia al proceso FFmpeg activo para poder matarlo (`child.kill()`) al cancelar.

### Flujo de compresión (por video, en `run_queue`)

Es **two-pass H.264** apuntando a un tamaño objetivo:
1. El bitrate de video se calcula desde la duración: `video_kbps = target_mb*8192/duration - AUDIO_KBPS`, con piso `MIN_VIDEO_KBPS` (si el piso se aplica, el resultado puede exceder el objetivo → se emite `warning`).
2. **Pass 1** (`-pass 1`, `-an`, salida a `null_device()`): análisis, cuenta como el primer 50% del progreso.
3. **Pass 2** (`-pass 2`, `+faststart`, AAC): codificación real, el segundo 50%.
4. Salida: `{stem}_comp.mp4` en la carpeta del original o en `out_dir` si se eligió.

El progreso se obtiene parseando `-progress pipe:1` de FFmpeg línea por línea en `run_ffmpeg_pass` (`out_time=` → `parse_out_time`), mapeado al rango `[base_frac, base_frac+span_frac]`.

### Detalles frágiles al editar

- **Filtro de escalado**: `scale=-2:min(ih\,{max_height})` — la coma va escapada (`\,`) porque la sintaxis de filtros de FFmpeg usa la coma como separador. No quitar el escape. Nunca agranda (usa `min(ih,...)`).
- **Passlog**: cada job usa un `-passlogfile` único en `temp_dir` con timestamp; `cleanup_passlog` borra los `.log`/`.log.mbtree` al terminar. Si añades pasadas, mantener limpieza.
- **Parámetros de calidad** (README los llama "ajustes rápidos"): `"-preset" "medium"`, `AUDIO_KBPS = 128`, `MIN_VIDEO_KBPS = 150`, tamaño objetivo por defecto `"90"` MB, resoluciones del ComboBox (1080p/720p/original) en `max_height_value()`.
- La detección de "cola terminada" en `poll()` es heurística (basada en que ya no haya jobs en `Processing`), no en el cierre del canal.

### Versiones pinneadas

Varias dependencias en `Cargo.toml` están fijadas con `=x.y.z` (`home`, `hashbrown`, `indexmap`, `ahash`, `url`) para compatibilidad con una versión antigua de Cargo. No actualizar a la ligera; si se necesita subir `eframe`/`egui`, quitar los pines y `cargo update` conscientemente.

## Entorno de desarrollo vs. target de distribución (importante)

**Se desarrolla en WSL/Linux, pero el binario final es para Windows.** Consecuencias:

- `cargo build`/`cargo run` en WSL produce un binario **Linux** (útil solo para probar en desarrollo). En Linux `rfd` (diálogos de archivo) requiere las libs de desarrollo de GTK3: `sudo apt install libgtk-3-dev pkg-config`.
- Para el `.exe` de Windows hay dos caminos:
  1. **Compilar en Windows** (nativo): instalar Rust en Windows y `cargo build --release` allí. `rfd` usa diálogos nativos, no necesita GTK.
  2. **Cross-compilar desde WSL**: `rustup target add x86_64-pc-windows-gnu` + `sudo apt install mingw-w64`, luego `cargo build --release --target x86_64-pc-windows-gnu`. El `.exe` queda en `target/x86_64-pc-windows-gnu/release/`.
- Ninguno de los dos está preparado aún en este WSL (solo hay target `x86_64-unknown-linux-gnu`, sin mingw).

Diferencias por plataforma en el código: `cfg!(windows)` decide los nombres de binarios de FFmpeg (`.exe`) y `null_device()` (`NUL` vs `/dev/null`); `open_containing_folder` tiene una rama por SO (explorer/xdg-open/open).
