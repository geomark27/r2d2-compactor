# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Qué es

App de escritorio nativa (Rust + egui/eframe) para comprimir **videos e imágenes** de evidencia pesados a un tamaño objetivo en MB antes de subirlos a SharePoint/Echo. No es un servicio ni tiene backend: orquesta **FFmpeg como proceso externo**. FFmpeg se distribuye **incluido** en el `.zip` de release (ver más abajo).

## Módulos (`src/`)

| Archivo | Responsabilidad |
|---------|-----------------|
| `main.rs` | Punto de entrada: declara los módulos y arranca la ventana eframe. |
| `model.rs` | Tipos de dominio: `Msg` (canal trabajo→UI, `Done` lleva la ruta de salida), `Job`, `JobState`, `MediaKind` (Video/Image + `from_path`). |
| `util.rs` | Helpers puros sin dependencias del resto: `fmt_size`, `parse_out_time`, `open_containing_folder`. |
| `ffmpeg.rs` | Localización e invocación de FFmpeg/FFprobe: `resolve_tool`, `which_in_path`, `null_device`, `probe_duration`, y el struct `Worker` con `run_pass` (con progreso, para video) y `run_quiet` (sin progreso, para imágenes). |
| `queue.rs` | Cola en el hilo de trabajo: `run_queue` enruta por `MediaKind` a `compress_video` (two-pass H.264) o `compress_image` (búsqueda binaria de calidad JPEG). `collect_pending` selecciona solo los `Queued` (idempotencia, con tests). `QueuedJob`, constantes de bitrate, `cleanup_passlog`. |
| `update.rs` | Auto-actualización desde GitHub Releases: `check_latest`, `self_update`, `is_newer`, enum `UpdateStatus`. Tiene tests unitarios de comparación de versiones. |
| `install.rs` | Integración con el SO: `ensure_start_menu_shortcut` crea el acceso directo del menú Inicio en Windows (vía PowerShell/WScript.Shell) para que la app sea buscable. No-op fuera de Windows. |
| `app.rs` | GUI egui: struct `App` (estado) + `impl eframe::App` (renderizado y polling). Acciones masivas de la cola (quitar terminados/todos). |

La documentación para el **usuario final** vive en `docs/` (p. ej. `docs/GUIA-DE-USUARIO.md`); cualquier guía o material para los compañeros va ahí, no en el README (que es más para instalación/desarrollo).

El logo está en `assets/icon.png` (PNG cuadrado con transparencia). Se usa en tres lugares, todos alimentados por ese único archivo:
1. **Icono de ventana / barra de tareas** (runtime): `main.rs` lo embebe con `include_bytes!` (`ICON_PNG`) y lo aplica con `ViewportBuilder::with_icon` vía `eframe::icon_data::from_png_bytes`.
2. **Logo del encabezado** (dentro de la UI): textura cargada de forma diferida en `App::ensure_logo`.
3. **Icono del `.exe` en el Explorador de Windows**: `build.rs` convierte el PNG a un `.ico` multi-resolución (crates `image` + `ico`) y lo incrusta como recurso con `winresource` (usa `x86_64-w64-mingw32-windres` al cross-compilar). Solo actúa cuando el target es Windows.

Para cambiar el logo: reemplazar `assets/icon.png` y recompilar; los tres usos se actualizan solos.

Dependencias entre módulos (sin ciclos): `app` → {`ffmpeg`, `queue`, `update`, `model`, `util`}; `queue` → {`ffmpeg`, `model`}; `ffmpeg` → {`model`, `util`}.

## Comandos

```bash
cargo build --release      # binario en target/release/r2d2-compactor(.exe)
cargo run                  # ejecutar en desarrollo
cargo check                # verificación rápida sin compilar el binario
cargo test                 # tests (versiones en update.rs, idempotencia en queue.rs)
make lint                  # cargo fmt --check + clippy -D warnings
```

`Cargo.toml` fija `clippy -D warnings` como estándar del proyecto (cero warnings). El `Makefile` tiene los atajos de build/lint/release.

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

### Flujo de compresión (en `run_queue`, enrutado por `MediaKind`)

**Video** (`compress_video`) — two-pass H.264 apuntando a un tamaño objetivo:
1. El bitrate se calcula desde la duración: `video_kbps = target_mb*8192/duration - AUDIO_KBPS`, con piso `MIN_VIDEO_KBPS` (si aplica, puede exceder el objetivo → `warning`).
2. **Pass 1** (`-pass 1`, `-an`, salida a `null_device()`): análisis, primer 50% del progreso.
3. **Pass 2** (`-pass 2`, `+faststart`, AAC): codificación real, segundo 50%.
4. Salida: `{stem}_comp.mp4`. El progreso sale de `-progress pipe:1` (`out_time=` → `parse_out_time`) mapeado a `[base_frac, base_frac+span_frac]`.

**Imagen** (`compress_image`) — a JPEG apuntando al mismo tamaño objetivo:
1. El peso de un JPEG decrece de forma monótona al subir `-q:v` (2 = mejor, 31 = peor), así que se hace **búsqueda binaria** del `q` más bajo (mejor calidad) cuyo tamaño ≤ objetivo, usando `Worker::run_quiet` (sin barra de progreso).
2. Si ni con `q=31` se logra, se codifica a `q=31` con un `warning` sugiriendo bajar la resolución.
3. Salida: `{stem}_comp.jpg`.

La ruta de salida la calcula `output_path` en `queue.rs` y viaja en `Msg::Done` hacia la UI (antes se recomputaba en `app.rs`).

### Detalles frágiles al editar

- **Filtro de escalado**: `scale=-2:min(ih\,{max_height})` — la coma va escapada (`\,`) porque la sintaxis de filtros de FFmpeg usa la coma como separador. No quitar el escape. Nunca agranda (usa `min(ih,...)`).
- **Passlog**: cada job usa un `-passlogfile` único en `temp_dir` con timestamp; `cleanup_passlog` borra los `.log`/`.log.mbtree` al terminar. Si añades pasadas, mantener limpieza.
- **Parámetros de calidad** (README los llama "ajustes rápidos"): `"-preset" "medium"`, `AUDIO_KBPS = 128`, `MIN_VIDEO_KBPS = 150`, tamaño objetivo por defecto `"90"` MB, resoluciones del ComboBox (1080p/720p/original) en `max_height_value()`.
- La detección de "cola terminada" en `poll()` es heurística (basada en que ya no haya jobs en `Processing`), no en el cierre del canal.

### Versiones pinneadas

Varias dependencias en `Cargo.toml` están fijadas con `=x.y.z` (`home`, `hashbrown`, `indexmap`, `ahash`, `url`) para compatibilidad con una versión antigua de Cargo. No actualizar a la ligera; si se necesita subir `eframe`/`egui`, quitar los pines y `cargo update` conscientemente.

## Auto-actualización y releases

La app se auto-actualiza desde **GitHub Releases** (repo `geomark27/r2d2-compactor`), replicando el patrón del CLI `gtt`:

- **Versión**: `env!("CARGO_PKG_VERSION")` — la fuente de verdad es el campo `version` de `Cargo.toml`. Los tags de git usan prefijo `v` (`v1.0.0`); `is_newer` los normaliza quitando la `v`.
- **Al arrancar**, `App::new()` lanza un hilo que llama `update::check_latest()` (GitHub API `/releases/latest`). El resultado se guarda en `Arc<Mutex<UpdateStatus>>` y la UI muestra un banner con botón "Actualizar ahora" si hay versión nueva. Un error al arrancar (sin internet) se degrada silenciosamente a `UpToDate` para no molestar.
- **Al pulsar el botón**, otro hilo descarga el asset + `checksums.txt`, verifica el SHA-256 y usa el crate `self-replace` para intercambiar el binario en uso (maneja el caso Windows del `.exe` bloqueado). Requiere reiniciar la app. **El updater descarga el `.exe` suelto, no el zip** — el FFmpeg ya está instalado desde la descarga inicial y no se toca.
- **El nombre del asset** (`asset_name()`) debe coincidir con lo que publica el `Makefile`: `r2d2-compactor-windows-amd64.exe` / `r2d2-compactor-linux-amd64`.

**Distribución con FFmpeg incluido**: el release de Windows es un **`.zip`** (`r2d2-compactor-windows-amd64.zip`) que contiene la app + una carpeta `ffmpeg/` con `ffmpeg.exe`/`ffprobe.exe`. Los binarios de FFmpeg viven en `vendor/ffmpeg-win/` (gitignored, ~195 MB); se bajan una vez con `make vendor-ffmpeg`.

**Publicar una versión** (necesita `gh` autenticado, toolchain de cross-compile y `make vendor-ffmpeg` hecho):

```bash
make release          # bump patch + build + zip con FFmpeg + checksums + tag + gh release
make release-minor    # bump minor
make release-major    # bump major
```

`_release` bumpea `Cargo.toml`, compila Linux, arma el zip de Windows (`dist-windows`), copia el `.exe` suelto, genera `checksums.txt` (zip + exe + linux), crea el tag `vX.Y.Z`, pushea y publica. Si cambia el nombre del repo o del binario, actualizar `REPO` y `asset_name()` en `update.rs`, y `BINARY`/`WIN_TARGET`/`VENDOR` en el `Makefile`.

## Entorno de desarrollo vs. target de distribución (importante)

**Se desarrolla en WSL/Linux, pero el binario final es para Windows.** Consecuencias:

- `cargo build`/`cargo run` en WSL produce un binario **Linux** (útil solo para probar en desarrollo). En Linux `rfd` (diálogos de archivo) requiere las libs de desarrollo de GTK3: `sudo apt install libgtk-3-dev pkg-config`.
- Para el `.exe` de Windows hay dos caminos:
  1. **Compilar en Windows** (nativo): instalar Rust en Windows y `cargo build --release` allí. `rfd` usa diálogos nativos, no necesita GTK.
  2. **Cross-compilar desde WSL** (ya configurado): `rustup target add x86_64-pc-windows-gnu` + `sudo apt install mingw-w64`, luego `make build-windows`. El `.exe` queda en `target/x86_64-pc-windows-gnu/release/`.
- Nota de cross-compile: `eframe 0.24` requiere que `winapi` tenga las features `winuser`/`windef` (declaradas en `Cargo.toml` bajo `[target.'cfg(windows)'.dependencies]`), y el `.exe` de release usa `windows_subsystem = "windows"` para no abrir consola.

Diferencias por plataforma en el código: `cfg!(windows)` decide los nombres de binarios de FFmpeg (`.exe`) y `null_device()` (`NUL` vs `/dev/null`); `open_containing_folder` tiene una rama por SO (explorer/xdg-open/open).
