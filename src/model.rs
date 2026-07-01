//! Tipos de dominio compartidos entre el hilo de UI y el hilo de trabajo.

use std::path::PathBuf;

/// Mensajes que el hilo de trabajo envía a la UI a través del canal.
pub enum Msg {
    Progress {
        id: u64,
        percent: f32,
        phase: &'static str,
    },
    Done {
        id: u64,
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

/// Un video en la cola, con su metadata y estado de progreso visible en la UI.
pub struct Job {
    pub id: u64,
    pub input: PathBuf,
    pub name: String,
    pub orig_bytes: Option<u64>,
    pub duration: Option<f64>,
    pub output: Option<PathBuf>,
    pub state: JobState,
    pub progress: f32,
    pub status: String,
}
