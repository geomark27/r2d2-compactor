//! Procesamiento de la cola de compresión en el hilo de trabajo.
//!
//! Videos: two-pass H.264 apuntando a un tamaño objetivo por duración.
//! Imágenes: búsqueda de la mejor calidad JPEG cuyo peso quede bajo el objetivo.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use crate::ffmpeg::Worker;
use crate::model::{MediaKind, Msg};

/// Bitrate de audio (kbps) usado en la codificación AAC.
const AUDIO_KBPS: i64 = 128;
/// Piso de bitrate de video (kbps) para videos muy largos frente al tamaño objetivo.
const MIN_VIDEO_KBPS: i64 = 150;

/// Un trabajo listo para procesar en el hilo de trabajo.
pub struct QueuedJob {
    pub id: u64,
    pub input: PathBuf,
    pub kind: MediaKind,
    /// Duración en segundos (solo relevante para video; 0 en imágenes).
    pub duration: f64,
}

/// Procesa la cola completa de forma secuencial, enrutando cada trabajo según su tipo.
pub fn run_queue(
    worker: Worker,
    jobs: Vec<QueuedJob>,
    target_mb: u32,
    max_height: u32,
    out_dir: Option<PathBuf>,
) {
    for job in jobs {
        if worker.cancel_flag.load(Ordering::SeqCst) {
            break;
        }
        let result = match job.kind {
            MediaKind::Video => compress_video(&worker, &job, target_mb, max_height, &out_dir),
            MediaKind::Image => compress_image(&worker, &job, target_mb, max_height, &out_dir),
        };
        if let Err(e) = result {
            // El centinela "__canceled__" no es un error real que reportar.
            if e != "__canceled__" {
                let _ = worker.tx.send(Msg::Error {
                    id: job.id,
                    message: e,
                });
            }
        }
    }
}

/// Ruta de salida `{stem}_comp.{ext}` en `out_dir` o junto al original.
fn output_path(out_dir: &Option<PathBuf>, input: &Path, ext: &str) -> PathBuf {
    let dir = out_dir.clone().unwrap_or_else(|| {
        input
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    });
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("archivo");
    dir.join(format!("{stem}_comp.{ext}"))
}

/// Filtro de escalado de FFmpeg que limita la altura sin agrandar nunca.
/// La coma va escapada (`\,`) porque FFmpeg la usa como separador de filtros.
fn scale_filter(max_height: u32) -> Option<String> {
    if max_height > 0 {
        Some(format!("scale=-2:min(ih\\,{max_height})"))
    } else {
        None
    }
}

/// Comprime un video con two-pass H.264 hacia un tamaño objetivo.
fn compress_video(
    worker: &Worker,
    job: &QueuedJob,
    target_mb: u32,
    max_height: u32,
    out_dir: &Option<PathBuf>,
) -> Result<(), String> {
    let id = job.id;
    let duration = job.duration;
    if duration <= 0.0 {
        return Err("no se pudo leer la duración del video.".into());
    }

    let total_kbit = target_mb as f64 * 8192.0;
    let mut video_kbps = (total_kbit / duration) as i64 - AUDIO_KBPS;
    let mut warning = None;
    if video_kbps < MIN_VIDEO_KBPS {
        video_kbps = MIN_VIDEO_KBPS;
        warning = Some("Video muy largo para ese tamaño objetivo: puede superarlo.".to_string());
    }

    let output = output_path(out_dir, &job.input, "mp4");
    let passlog = std::env::temp_dir().join(format!(
        "cev_{id}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    ));
    let passlog_str = passlog.to_string_lossy().to_string();
    let scale = scale_filter(max_height);
    let input = job.input.to_string_lossy().to_string();

    // ---- Pass 1: análisis (sin salida real) ----
    let mut args1: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        input.clone(),
        "-c:v".into(),
        "libx264".into(),
        "-b:v".into(),
        format!("{video_kbps}k"),
        "-pass".into(),
        "1".into(),
        "-passlogfile".into(),
        passlog_str.clone(),
        "-preset".into(),
        "medium".into(),
        "-an".into(),
    ];
    if let Some(sf) = &scale {
        args1.push("-vf".into());
        args1.push(sf.clone());
    }
    args1.push("-f".into());
    args1.push("null".into());
    args1.push(crate::ffmpeg::null_device().into());
    args1.push("-progress".into());
    args1.push("pipe:1".into());
    args1.push("-nostats".into());

    if let Err(e) = worker.run_pass(&args1, duration, 0.0, 0.5, id, "Analizando (1/2)") {
        cleanup_passlog(&passlog_str);
        return Err(e);
    }

    // ---- Pass 2: codificación real ----
    let mut args2: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        input,
        "-c:v".into(),
        "libx264".into(),
        "-b:v".into(),
        format!("{video_kbps}k"),
        "-pass".into(),
        "2".into(),
        "-passlogfile".into(),
        passlog_str.clone(),
        "-preset".into(),
        "medium".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        format!("{AUDIO_KBPS}k"),
        "-movflags".into(),
        "+faststart".into(),
    ];
    if let Some(sf) = &scale {
        args2.push("-vf".into());
        args2.push(sf.clone());
    }
    args2.push(output.to_string_lossy().to_string());
    args2.push("-progress".into());
    args2.push("pipe:1".into());
    args2.push("-nostats".into());

    let r2 = worker.run_pass(&args2, duration, 0.5, 0.5, id, "Comprimiendo (2/2)");
    cleanup_passlog(&passlog_str);

    match r2 {
        Ok(()) => {
            let final_bytes = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
            let _ = worker.tx.send(Msg::Done {
                id,
                output,
                final_bytes,
                warning,
            });
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&output);
            Err(e)
        }
    }
}

/// Comprime una imagen a JPEG buscando la mejor calidad que quede bajo el objetivo.
///
/// Como el peso de un JPEG decrece de forma monótona al subir `-q:v` (2 = mejor,
/// 31 = peor), se hace una búsqueda binaria del `q` más bajo (mejor calidad) cuyo
/// tamaño no supere el objetivo.
fn compress_image(
    worker: &Worker,
    job: &QueuedJob,
    target_mb: u32,
    max_height: u32,
    out_dir: &Option<PathBuf>,
) -> Result<(), String> {
    let id = job.id;
    let output = output_path(out_dir, &job.input, "jpg");
    let target_bytes = target_mb as u64 * 1024 * 1024;
    let scale = scale_filter(max_height);
    let input = job.input.to_string_lossy().to_string();
    let tmp = std::env::temp_dir().join(format!("r2d2_img_{id}.jpg"));

    let _ = worker.tx.send(Msg::Progress {
        id,
        percent: 20.0,
        phase: "Comprimiendo imagen",
    });

    // Codifica a `dst` con la calidad `q` y devuelve el tamaño resultante.
    let encode = |q: i32, dst: &Path| -> Result<u64, String> {
        let mut args = vec!["-y".into(), "-i".into(), input.clone()];
        if let Some(sf) = &scale {
            args.push("-vf".into());
            args.push(sf.clone());
        }
        args.push("-q:v".into());
        args.push(q.to_string());
        args.push("-pix_fmt".into());
        args.push("yuvj420p".into());
        args.push(dst.to_string_lossy().to_string());
        worker.run_quiet(&args)?;
        Ok(std::fs::metadata(dst).map(|m| m.len()).unwrap_or(u64::MAX))
    };

    // Búsqueda binaria del q más bajo (mejor calidad) cuyo tamaño <= objetivo.
    let mut best_q: Option<i32> = None;
    let (mut lo, mut hi) = (2, 31);
    while lo <= hi {
        let mid = (lo + hi) / 2;
        let size = encode(mid, &tmp)?;
        if size <= target_bytes {
            best_q = Some(mid);
            hi = mid - 1;
        } else {
            lo = mid + 1;
        }
    }

    let _ = worker.tx.send(Msg::Progress {
        id,
        percent: 80.0,
        phase: "Comprimiendo imagen",
    });

    let (final_q, warning) = match best_q {
        Some(q) => (q, None),
        None => (
            31,
            Some(
                "La imagen no baja al tamaño objetivo ni con máxima compresión; \
                 prueba una resolución máxima menor."
                    .to_string(),
            ),
        ),
    };

    let final_bytes = encode(final_q, &output)?;
    let _ = std::fs::remove_file(&tmp);
    let _ = worker.tx.send(Msg::Done {
        id,
        output,
        final_bytes,
        warning,
    });
    Ok(())
}

/// Borra los archivos de log temporales que genera el two-pass de FFmpeg.
fn cleanup_passlog(prefix: &str) {
    let _ = std::fs::remove_file(format!("{prefix}-0.log"));
    let _ = std::fs::remove_file(format!("{prefix}-0.log.mbtree"));
}
