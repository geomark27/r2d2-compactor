//! Localización e invocación de los binarios externos de FFmpeg/FFprobe.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::model::Msg;
use crate::util::parse_out_time;

/// Evita que Windows abra una ventana de consola por cada proceso de FFmpeg.
/// En otras plataformas no hace nada.
#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}
#[cfg(not(windows))]
fn hide_console(_cmd: &mut Command) {}

/// Resuelve la ruta de una herramienta: primero la carpeta `ffmpeg/` junto al
/// ejecutable, y como último recurso el nombre a secas (para buscarla en PATH).
pub fn resolve_tool(name_win: &str, name_unix: &str) -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let bundled = exe_dir
        .join("ffmpeg")
        .join(if cfg!(windows) { name_win } else { name_unix });
    if bundled.exists() {
        return bundled;
    }
    // Fallback: buscarlo en el PATH del sistema (útil en desarrollo)
    PathBuf::from(if cfg!(windows) { name_win } else { name_unix })
}

/// Busca un binario por nombre a lo largo de las entradas de la variable PATH.
pub fn which_in_path(name: &Path) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Dispositivo nulo del sistema, destino de la salida en la primera pasada.
pub fn null_device() -> &'static str {
    if cfg!(windows) {
        "NUL"
    } else {
        "/dev/null"
    }
}

/// Obtiene la duración del video en segundos usando ffprobe.
pub fn probe_duration(ffprobe: &Path, input: &Path) -> Result<f64, String> {
    let mut cmd = Command::new(ffprobe);
    cmd.args([
        "-v",
        "error",
        "-show_entries",
        "format=duration",
        "-of",
        "default=noprint_wrappers=1:nokey=1",
    ])
    .arg(input);
    hide_console(&mut cmd);
    let out = cmd
        .output()
        .map_err(|e| format!("no se pudo ejecutar ffprobe: {e}"))?;

    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    text.parse::<f64>()
        .map_err(|_| "duración inválida".to_string())
}

/// Contexto compartido del hilo de trabajo: binario de FFmpeg, canal hacia la
/// UI y señales de cancelación. Estos cuatro valores siempre viajan juntos, así
/// que se agrupan en un solo tipo para pasarlos por la cola de compresión.
pub struct Worker {
    pub ffmpeg: PathBuf,
    pub tx: Sender<Msg>,
    pub cancel_flag: Arc<AtomicBool>,
    pub current_child: Arc<Mutex<Option<Child>>>,
    /// Archivo donde se vuelca la salida de diagnóstico (stderr) de FFmpeg.
    pub log_path: PathBuf,
}

impl Worker {
    /// Abre el log en modo "append" para usarlo como stderr de FFmpeg. Si no se
    /// puede abrir, cae a descartar la salida (nunca debe romper la compresión).
    fn log_stderr(&self) -> Stdio {
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
        {
            Ok(f) => Stdio::from(f),
            Err(_) => Stdio::null(),
        }
    }
}

impl Worker {
    /// Ejecuta una pasada de FFmpeg reportando progreso al canal de la UI.
    ///
    /// El progreso se mapea al rango `[base_frac, base_frac + span_frac]` para
    /// componer varias pasadas en una única barra de 0 a 100%.
    pub fn run_pass(
        &self,
        args: &[String],
        duration: f64,
        base_frac: f64,
        span_frac: f64,
        id: u64,
        phase: &'static str,
    ) -> Result<(), String> {
        let mut cmd = Command::new(&self.ffmpeg);
        cmd.args(args)
            .stdout(Stdio::piped())
            .stderr(self.log_stderr())
            .stdin(Stdio::null());
        hide_console(&mut cmd);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("no se pudo iniciar FFmpeg: {e}"))?;
        let stdout = child.stdout.take().ok_or("sin stdout")?;
        // Registra el proceso para poder matarlo al cancelar o cerrar la app.
        *self.current_child.lock().unwrap() = Some(child);

        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if self.cancel_flag.load(Ordering::SeqCst) {
                self.kill_current();
                let _ = self.tx.send(Msg::Canceled { id });
                return Err("__canceled__".to_string());
            }
            if let Some(rest) = line.strip_prefix("out_time=") {
                if let Some(sec) = parse_out_time(rest) {
                    if duration > 0.0 {
                        let frac = (sec / duration).clamp(0.0, 1.0);
                        let percent = ((base_frac + frac * span_frac) * 100.0) as f32;
                        let _ = self.tx.send(Msg::Progress { id, percent, phase });
                    }
                }
            }
        }

        // El stream terminó: FFmpeg acabó (o fue detenido desde otro hilo).
        let Some(mut child) = self.current_child.lock().unwrap().take() else {
            let _ = self.tx.send(Msg::Canceled { id });
            return Err("__canceled__".to_string());
        };
        let status = child
            .wait()
            .map_err(|e| format!("error esperando FFmpeg: {e}"))?;
        if self.cancel_flag.load(Ordering::SeqCst) {
            let _ = self.tx.send(Msg::Canceled { id });
            return Err("__canceled__".to_string());
        }
        if !status.success() {
            return Err(format!("FFmpeg terminó con código {:?}", status.code()));
        }
        Ok(())
    }

    /// Mata y limpia (reap) el proceso de FFmpeg activo, si lo hay.
    fn kill_current(&self) {
        if let Some(mut child) = self.current_child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Ejecuta FFmpeg hasta terminar sin reportar progreso (para operaciones
    /// rápidas como codificar una imagen). Sondea el estado para poder cancelar
    /// o cerrar la app aunque no haya salida de progreso que leer.
    pub fn run_quiet(&self, args: &[String]) -> Result<(), String> {
        if self.cancel_flag.load(Ordering::SeqCst) {
            return Err("__canceled__".to_string());
        }
        let mut cmd = Command::new(&self.ffmpeg);
        cmd.args(args)
            .stdout(Stdio::null())
            .stderr(self.log_stderr())
            .stdin(Stdio::null());
        hide_console(&mut cmd);
        let child = cmd
            .spawn()
            .map_err(|e| format!("no se pudo iniciar FFmpeg: {e}"))?;
        *self.current_child.lock().unwrap() = Some(child);

        loop {
            if self.cancel_flag.load(Ordering::SeqCst) {
                self.kill_current();
                return Err("__canceled__".to_string());
            }
            let mut guard = self.current_child.lock().unwrap();
            match guard.as_mut() {
                Some(child) => match child.try_wait() {
                    Ok(Some(status)) => {
                        let _ = guard.take();
                        return if status.success() {
                            Ok(())
                        } else {
                            Err(format!("FFmpeg terminó con código {:?}", status.code()))
                        };
                    }
                    Ok(None) => {} // sigue corriendo
                    Err(e) => {
                        let _ = guard.take();
                        return Err(format!("error esperando FFmpeg: {e}"));
                    }
                },
                None => return Err("__canceled__".to_string()), // detenido desde otro hilo
            }
            drop(guard);
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}
