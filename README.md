# R2D2 Compactor (Rust + egui)

App de escritorio **nativa** para comprimir **videos e imágenes** de evidencia pesados antes de subirlos a Echo. Herramienta totalmente independiente del proyecto Laravel: el inspector la abre, arrastra los archivos, los comprime y sube el resultado a mano.

Cero JavaScript, cero webview, cero runtime que instalar. Es un binario nativo (Rust + [egui](https://github.com/emilk/egui)/eframe) que usa **FFmpeg** (incluido en la descarga) como motor de compresión.

## Para usuarios (tus compañeros)

1. Descarga el instalador **`r2d2-compactor-setup.exe`** desde la [pestaña Releases del repo](https://github.com/geomark27/r2d2-compactor/releases).
2. Doble clic y sigue el asistente (Siguiente → Instalar → Finalizar). Trae todo incluido (FFmpeg); instala **por usuario** (sin permisos de administrador), crea el acceso del menú Inicio y se desinstala desde "Agregar o quitar programas".

Alternativa **portable**: el `r2d2-compactor-windows-amd64.zip` — descomprimir completo y abrir `r2d2-compactor.exe` (manteniendo el `.exe` y la carpeta `ffmpeg/` juntos).

📖 **Guía de usuario completa (paso a paso, con solución de problemas):** [`docs/GUIA-DE-USUARIO.md`](docs/GUIA-DE-USUARIO.md)

### Actualizaciones automáticas

Al abrir, la app comprueba sola si hay una versión más reciente. Si la hay, muestra un aviso con el botón **"Actualizar ahora"**: se descarga, verifica su integridad (SHA-256) y se reemplaza a sí misma. Solo hay que **cerrarla y volver a abrirla**. No hay que volver a descargar el zip ni tocar la carpeta `ffmpeg/`.

## Qué comprime y cómo

- **Videos** (MP4, MOV, AVI, MKV, M4V, WMV): **two-pass H.264 (libx264)** apuntando a un tamaño objetivo en MB. El bitrate se calcula según la duración, así el resultado cae de forma predecible bajo el límite. Audio AAC + `faststart` para máxima compatibilidad con SharePoint/navegadores.
- **Imágenes** (JPG, PNG, WEBP, BMP, TIFF): se recomprimen a JPEG buscando automáticamente la mejor calidad que quede bajo el mismo **tamaño objetivo**.
- **Escalado opcional a 1080p / 720p**: nunca agranda, solo reduce si la fuente es mayor.

## Uso

1. Arrastra uno o varios archivos a la ventana (o usa "Elegir archivos…"). Puedes mezclar videos e imágenes.
2. Ajusta el **tamaño objetivo** (MB) y la **resolución máxima** si quieres.
3. Por defecto guarda junto al original con el sufijo `_comp` (`.mp4` para video, `.jpg` para imagen); o elige otra carpeta de salida. El original **no se modifica**.
4. **Comprimir todo**. Al terminar, "Ver archivo" abre la carpeta con el resultado.

---

## Para desarrolladores

### Compilar y probar

Requiere el toolchain de Rust ([rustup.rs](https://rustup.rs)). En desarrollo la app busca FFmpeg en el `PATH` del sistema como fallback, así que basta con tener `ffmpeg`/`ffprobe` instalados para probar.

```bash
cargo build --release      # binario en target/release/r2d2-compactor(.exe)
cargo run                  # ejecutar en desarrollo
cargo test                 # tests
make lint                  # cargo fmt --check + clippy -D warnings
```

> **Nota sobre el entorno**: el desarrollo es en WSL/Linux pero el target real es **Windows**. Un `cargo build` en Linux produce un binario Linux (y `rfd` necesita GTK3: `sudo apt install libgtk-3-dev pkg-config`).

### Publicar un release

Setup único para cross-compilar a Windows y empaquetar FFmpeg:

```bash
rustup target add x86_64-pc-windows-gnu
sudo apt install mingw-w64
make vendor-ffmpeg          # baja FFmpeg de Windows a vendor/ (una sola vez, ~110 MB)
```

Luego, con `gh` autenticado:

```bash
make release          # bump patch + build + zip con FFmpeg + checksums + tag + GitHub Release
make release-minor    # bump minor
make release-major    # bump major
```

Cada release publica cuatro assets: el **instalador `setup.exe`** (asistente NSIS con FFmpeg incluido — lo que descargan los usuarios), el **`.zip` de Windows** (versión portable), el **`.exe` suelto** (lo usa la auto-actualización) y el binario Linux, más `checksums.txt`. Desde que se publica, las apps instaladas detectan la nueva versión al abrir.

Requisito extra para el instalador: `sudo apt install nsis` (el script está en `installer/installer.nsi`; también `make dist-installer` lo genera suelto).

## Notas técnicas

- La versión mostrada y comparada sale de `Cargo.toml` (`CARGO_PKG_VERSION`); los tags de git usan prefijo `v` (`v1.0.0`). Por eso `make release` bumpea `Cargo.toml` **antes** de compilar.
- Las versiones de algunas dependencias en `Cargo.toml` están fijadas (`=x.y.z`) por compatibilidad; si quieres subir `eframe`/`egui`, quita los pines y corre `cargo update`.
- `rfd` (diálogos de archivo) usa GTK3 en Linux; en Windows usa los diálogos nativos, no necesita nada adicional.
- El progreso de video se lee parseando la salida `-progress pipe:1` de FFmpeg — es el avance real de cada pasada, no una barra falsa.
- Las imágenes se comprimen con una búsqueda binaria de la calidad JPEG (`-q:v` 2..31) hasta quedar bajo el tamaño objetivo; si ni con máxima compresión se logra, se avisa y se sugiere bajar la resolución.

## Ajustes rápidos de compresión (en `src/queue.rs`)

- `"-preset".into(), "medium".into()` → cámbialo a `"slow"` para más compresión (más lento) o `"fast"` para más velocidad.
- `AUDIO_KBPS` (128) → bitrate de audio.
- `MIN_VIDEO_KBPS` (150) → piso de bitrate para videos muy largos relativos al tamaño objetivo.

## Aprende cómo funciona

📚 Documento explicativo de la lógica de compresión y optimización (bitrate, two-pass, códecs H.264, CRF, JPEG…), conectado con el código: [`docs/COMPRESION-EXPLICADA.md`](docs/COMPRESION-EXPLICADA.md)
