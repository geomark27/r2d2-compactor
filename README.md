# R2D2 Compactor (Rust + egui)

App de escritorio **nativa** para comprimir videos de evidencia pesados antes de subirlos a Echo. Herramienta totalmente independiente del proyecto Laravel: el inspector la abre, arrastra el video, lo comprime y sube el resultado a mano.

Cero JavaScript, cero webview, cero runtime que instalar. Es un único binario nativo (Rust + [egui](https://github.com/emilk/egui)/eframe) que llama a **FFmpeg** como proceso externo.

## Para usuarios (tus compañeros)

1. Descarga `r2d2-compactor-windows-amd64.exe` desde la [pestaña Releases del repo](https://github.com/geomark27/r2d2-compactor/releases) y renómbralo a `r2d2-compactor.exe` (opcional).
2. Junto al `.exe`, crea una carpeta `ffmpeg/` con `ffmpeg.exe` y `ffprobe.exe` (ver [Paso 1](#paso-1--conseguir-ffmpeg-una-sola-vez)). Esto se hace **una sola vez**.
3. Abre la app con doble clic.

### Actualizaciones automáticas

Al abrir, la app comprueba sola si hay una versión más reciente. Si la hay, muestra un aviso con el botón **"Actualizar ahora"**: se descarga, verifica su integridad (SHA-256) y se reemplaza a sí misma. Solo hay que **cerrarla y volver a abrirla** para usar la versión nueva. No hay que descargar nada a mano ni tocar la carpeta `ffmpeg/`.

## Cómo comprime

- **Two-pass H.264 (libx264)** apuntando a un tamaño objetivo en MB. El bitrate se calcula a partir de la duración, así el resultado cae de forma predecible bajo el límite (90 MB por defecto, con margen bajo el tope de 100 MB de SharePoint).
- **H.264 + AAC + `faststart`**: máxima compatibilidad con la previsualización de SharePoint y navegadores.
- **Escalado opcional a 1080p / 720p**: nunca agranda, solo reduce si la fuente es mayor.

## Uso

1. Arrastra uno o varios videos a la ventana (o usa "Elegir videos…").
2. Ajusta el **tamaño objetivo** (MB) y la **resolución máxima** si quieres.
3. Por defecto guarda junto al original con el sufijo `_comp.mp4`; o elige otra carpeta de salida.
4. **Comprimir todo**. Al terminar, "Ver archivo" abre la carpeta con el resultado.

---

## Para desarrolladores

### Paso 1 — Conseguir FFmpeg (una sola vez)

Descarga un build oficial de Windows (gratis) y coloca **ffmpeg.exe** y **ffprobe.exe** en una carpeta `ffmpeg/` junto al `.exe` final:

- https://www.gyan.dev/ffmpeg/builds/ → *release essentials*
- o https://github.com/BtbN/FFmpeg-Builds/releases

La app también los busca en el `PATH` del sistema como fallback (útil en desarrollo).

### Paso 2 — Compilar

Requiere el toolchain de Rust ([rustup.rs](https://rustup.rs), instala `cargo` y `rustc`).

```bash
cargo build --release      # binario en target/release/r2d2-compactor(.exe)
cargo run                  # ejecutar en desarrollo
cargo test                 # tests
make lint                  # cargo fmt --check + clippy -D warnings
```

> **Nota sobre el entorno**: el desarrollo es en WSL/Linux pero el target real es **Windows**. Un `cargo build` en Linux produce un binario Linux (y `rfd` necesita GTK3: `sudo apt install libgtk-3-dev pkg-config`). Para el `.exe` de Windows ver la sección de Releases.

### Paso 3 — Publicar un release

Setup único para cross-compilar a Windows desde WSL:

```bash
rustup target add x86_64-pc-windows-gnu
sudo apt install mingw-w64
```

Luego, con `gh` autenticado:

```bash
make release          # bump patch + build Linux/Windows + checksums + tag + GitHub Release
make release-minor    # bump minor
make release-major    # bump major
```

Esto sube los binarios y un `checksums.txt` a GitHub Releases. Desde ese momento, las apps instaladas de tus compañeros detectarán la nueva versión al abrir.

## Notas técnicas

- La versión mostrada y comparada sale de `Cargo.toml` (`CARGO_PKG_VERSION`); los tags de git usan prefijo `v` (`v1.0.0`). Por eso `make release` bumpea `Cargo.toml` **antes** de compilar.
- Las versiones de algunas dependencias en `Cargo.toml` están fijadas (`=x.y.z`) por compatibilidad; si quieres subir `eframe`/`egui`, quita los pines y corre `cargo update`.
- `rfd` (diálogos de archivo) usa GTK3 en Linux; en Windows usa los diálogos nativos, no necesita nada adicional.
- El progreso se lee parseando la salida `-progress pipe:1` de FFmpeg — es el avance real de cada pasada, no una barra falsa.

## Ajustes rápidos de compresión (en `src/queue.rs`)

- `"-preset".into(), "medium".into()` → cámbialo a `"slow"` para más compresión (más lento) o `"fast"` para más velocidad.
- `AUDIO_KBPS` (128) → bitrate de audio.
- `MIN_VIDEO_KBPS` (150) → piso de bitrate para videos muy largos relativos al tamaño objetivo.
