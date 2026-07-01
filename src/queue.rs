//! Procesamiento de la cola de compresión (two-pass H.264) en el hilo de trabajo.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use crate::ffmpeg::{null_device, Worker};
use crate::model::Msg;

/// Bitrate de audio (kbps) usado en la codificación AAC.
const AUDIO_KBPS: i64 = 128;
/// Piso de bitrate de video (kbps) para videos muy largos frente al tamaño objetivo.
const MIN_VIDEO_KBPS: i64 = 150;

/// Procesa la cola completa de videos de forma secuencial en el hilo de trabajo.
///
/// Cada video pasa por dos pasadas de FFmpeg: análisis (`-pass 1`) y codificación
/// real (`-pass 2`), reportando progreso y resultado por el canal del `worker`.
pub fn run_queue(
    worker: Worker,
    jobs: Vec<(u64, PathBuf, f64)>,
    target_mb: u32,
    max_height: u32,
    out_dir: Option<PathBuf>,
) {
    for (id, input, duration) in jobs {
        if worker.cancel_flag.load(Ordering::SeqCst) {
            break;
        }
        if duration <= 0.0 {
            let _ = worker.tx.send(Msg::Error {
                id,
                message: "no se pudo leer la duración del video.".into(),
            });
            continue;
        }

        let total_kbit = target_mb as f64 * 8192.0;
        let mut video_kbps = (total_kbit / duration) as i64 - AUDIO_KBPS;
        let mut warning = None;
        if video_kbps < MIN_VIDEO_KBPS {
            video_kbps = MIN_VIDEO_KBPS;
            warning =
                Some("Video muy largo para ese tamaño objetivo: puede superarlo.".to_string());
        }

        let dir = out_dir.clone().unwrap_or_else(|| {
            input
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."))
        });
        let stem = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("video");
        let output = dir.join(format!("{stem}_comp.mp4"));

        let passlog = std::env::temp_dir().join(format!(
            "cev_{id}_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let passlog_str = passlog.to_string_lossy().to_string();

        let scale_arg = if max_height > 0 {
            Some(format!("scale=-2:min(ih\\,{max_height})"))
        } else {
            None
        };

        // ---- Pass 1: análisis (sin salida real) ----
        let mut args1: Vec<String> = vec![
            "-y".into(),
            "-i".into(),
            input.to_string_lossy().to_string(),
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
        if let Some(sf) = &scale_arg {
            args1.push("-vf".into());
            args1.push(sf.clone());
        }
        args1.push("-f".into());
        args1.push("null".into());
        args1.push(null_device().into());
        args1.push("-progress".into());
        args1.push("pipe:1".into());
        args1.push("-nostats".into());

        let r1 = worker.run_pass(&args1, duration, 0.0, 0.5, id, "Analizando (1/2)");
        if let Err(e) = r1 {
            if e != "__canceled__" {
                let _ = worker.tx.send(Msg::Error { id, message: e });
            }
            cleanup_passlog(&passlog_str);
            continue;
        }

        // ---- Pass 2: codificación real ----
        let mut args2: Vec<String> = vec![
            "-y".into(),
            "-i".into(),
            input.to_string_lossy().to_string(),
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
        if let Some(sf) = &scale_arg {
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
                    final_bytes,
                    warning,
                });
            }
            Err(e) => {
                let _ = std::fs::remove_file(&output);
                if e != "__canceled__" {
                    let _ = worker.tx.send(Msg::Error { id, message: e });
                }
            }
        }
    }
}

/// Borra los archivos de log temporales que genera el two-pass de FFmpeg.
fn cleanup_passlog(prefix: &str) {
    let _ = std::fs::remove_file(format!("{prefix}-0.log"));
    let _ = std::fs::remove_file(format!("{prefix}-0.log.mbtree"));
}
