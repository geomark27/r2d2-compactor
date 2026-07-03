//! Tipos de dominio compartidos entre el hilo de UI y el hilo de trabajo.

use std::path::{Path, PathBuf};

/// Tipo de archivo que se está comprimiendo. Determina la ruta de compresión.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MediaKind {
    Video,
    Image,
}

impl MediaKind {
    /* Detecta el tipo por la extensión del archivo. Lo desconocido se trata
    como video (FFmpeg dará un error claro si no puede procesarlo). */
    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "jpg" | "jpeg" | "png" | "webp" | "bmp" | "tif" | "tiff" | "heic" => MediaKind::Image,
            _ => MediaKind::Video,
        }
    }
}

/// Mensajes que el hilo de trabajo envía a la UI a través del canal.
pub enum Msg {
    Progress {
        id: u64,
        percent: f32,
        phase: &'static str,
    },
    Done {
        id: u64,
        output: PathBuf,
        final_bytes: u64,
        warning: Option<String>,
    },
    Error {
        id: u64,
        message: String,
    },
    Canceled {
        id: u64,
    },
    /// El hilo de trabajo terminó la cola completa (haya sido con éxito,
    /// con errores o por cancelación). Es la única señal que apaga `running`.
    Finished,
}

/// Estado de cada trabajo de compresión dentro de la cola.
#[derive(Clone, PartialEq, Debug)]
#[allow(dead_code)]
pub enum JobState {
    Queued,
    Processing,
    Done,
    Error,
    Canceled,
}

/// Un archivo (video o imagen) en la cola, con su metadata y progreso visible.
pub struct Job {
    pub id: u64,
    pub input: PathBuf,
    pub name: String,
    pub kind: MediaKind,
    pub orig_bytes: Option<u64>,
    pub duration: Option<f64>,
    pub output: Option<PathBuf>,
    pub state: JobState,
    pub progress: f32,
    pub status: String,
}
