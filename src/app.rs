//! Aplicación egui: estado de la UI, cola de trabajos y renderizado.

use eframe::egui;
use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::ffmpeg::{probe_duration, resolve_tool, which_in_path, Worker};
use crate::model::{Job, JobState, MediaKind, Msg};
use crate::queue::{collect_pending, run_queue};
use crate::update::{self, UpdateStatus};
use crate::util::{fmt_size, open_containing_folder, open_file};

/// Estado global de la aplicación de escritorio.
pub struct App {
    jobs: Vec<Job>,
    next_id: u64,
    target_mb: String,
    max_height_idx: usize,
    out_dir: Option<PathBuf>,
    running: bool,
    cancel_flag: Arc<AtomicBool>,
    current_child: Arc<Mutex<Option<Child>>>,
    rx: Option<Receiver<Msg>>,
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
    ffmpeg_missing: bool,
    update_status: Arc<Mutex<UpdateStatus>>,
    /// Textura del logo, cargada de forma diferida en el primer frame.
    logo: Option<egui::TextureHandle>,
    /// Ruta del archivo de registro (stderr de FFmpeg de la última corrida).
    log_path: PathBuf,
}

impl App {
    pub fn new() -> Self {
        let ffmpeg = resolve_tool("ffmpeg.exe", "ffmpeg");
        let ffprobe = resolve_tool("ffprobe.exe", "ffprobe");
        let missing = !ffmpeg.exists() && which_in_path(&ffmpeg).is_none();

        // Crea el acceso directo del menú Inicio (Windows) para poder buscar la
        // app por su nombre. En segundo plano y best-effort: si falla, se ignora.
        std::thread::spawn(crate::install::ensure_start_menu_shortcut);

        // Comprueba en segundo plano si hay una versión nueva, sin bloquear la UI.
        let update_status = Arc::new(Mutex::new(UpdateStatus::Checking));
        {
            let status = update_status.clone();
            std::thread::spawn(move || {
                // Un error al arrancar (sin internet, límite de API) se trata como
                // "al día" para no molestar con un aviso rojo.
                let next = match update::check_latest() {
                    Ok(Some(version)) => UpdateStatus::Available(version),
                    _ => UpdateStatus::UpToDate,
                };
                *status.lock().unwrap() = next;
            });
        }

        Self {
            jobs: Vec::new(),
            next_id: 1,
            target_mb: "90".to_string(),
            max_height_idx: 0,
            out_dir: None,
            running: false,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            current_child: Arc::new(Mutex::new(None)),
            rx: None,
            ffmpeg,
            ffprobe,
            ffmpeg_missing: missing,
            update_status,
            logo: None,
            log_path: std::env::temp_dir().join("r2d2-compactor.log"),
        }
    }

    /// Carga el logo como textura la primera vez (necesita el `Context` de egui,
    /// que no existe todavía en `new`). Si el PNG no es válido, se omite.
    fn ensure_logo(&mut self, ctx: &egui::Context) {
        if self.logo.is_some() {
            return;
        }
        if let Ok(icon) = eframe::icon_data::from_png_bytes(crate::ICON_PNG) {
            let image = egui::ColorImage::from(icon);
            self.logo = Some(ctx.load_texture("logo", image, egui::TextureOptions::LINEAR));
        }
    }

    /// Dibuja el banner de actualización según el estado actual y gestiona el
    /// botón "Actualizar ahora" (que lanza la descarga en un hilo aparte).
    fn show_update_banner(&mut self, ui: &mut egui::Ui) {
        let status = self.update_status.lock().unwrap().clone();
        match status {
            UpdateStatus::Available(version) => {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        egui::Color32::from_rgb(96, 205, 128),
                        format!("Nueva versión {version} disponible."),
                    );
                    if ui.button("Actualizar ahora").clicked() {
                        *self.update_status.lock().unwrap() = UpdateStatus::Downloading;
                        let status = self.update_status.clone();
                        std::thread::spawn(move || {
                            let next = match update::self_update(&version) {
                                Ok(()) => UpdateStatus::Updated(version.clone()),
                                Err(e) => UpdateStatus::Error(e),
                            };
                            *status.lock().unwrap() = next;
                        });
                    }
                });
            }
            UpdateStatus::Downloading => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Descargando actualización…");
                });
            }
            UpdateStatus::Updated(version) => {
                ui.colored_label(
                    egui::Color32::from_rgb(96, 205, 128),
                    format!("Actualizado a {version}. Cierra y vuelve a abrir la app."),
                );
            }
            UpdateStatus::Error(e) => {
                ui.colored_label(
                    egui::Color32::from_rgb(248, 113, 113),
                    format!("No se pudo actualizar: {e}"),
                );
            }
            UpdateStatus::Checking | UpdateStatus::UpToDate => {}
        }
    }

    /// `true` si hay una comprobación o descarga de actualización en curso.
    fn update_in_progress(&self) -> bool {
        matches!(
            *self.update_status.lock().unwrap(),
            UpdateStatus::Checking | UpdateStatus::Downloading
        )
    }

    /// Detiene y limpia el proceso de FFmpeg en curso, si lo hay. Se usa al
    /// cancelar y al cerrar la app para no dejar procesos huérfanos.
    fn stop_current_child(&self) {
        if let Some(mut child) = self.current_child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Añade un archivo (video o imagen) a la cola, evitando duplicados.
    fn add_file(&mut self, path: PathBuf) {
        if self.jobs.iter().any(|j| j.input == path) {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("archivo")
            .to_string();
        let kind = MediaKind::from_path(&path);
        let orig_bytes = std::fs::metadata(&path).ok().map(|m| m.len());
        // Solo los videos necesitan la duración (para calcular el bitrate).
        let duration = match kind {
            MediaKind::Video => probe_duration(&self.ffprobe, &path).ok(),
            MediaKind::Image => None,
        };
        self.jobs.push(Job {
            id,
            input: path,
            name,
            kind,
            orig_bytes,
            duration,
            output: None,
            state: JobState::Queued,
            progress: 0.0,
            status: "En cola".to_string(),
        });
    }

    /// Altura máxima (px) según la opción de resolución seleccionada; 0 = no escalar.
    fn max_height_value(&self) -> u32 {
        match self.max_height_idx {
            0 => 1080,
            1 => 720,
            _ => 0,
        }
    }

    /// Lanza el hilo de trabajo que procesa todos los videos en cola.
    fn start_run(&mut self) {
        if self.running {
            return;
        }
        let target_mb: u32 = self.target_mb.trim().parse().unwrap_or(90).max(5);
        let max_height = self.max_height_value();
        // Solo los trabajos en cola (idempotente: no re-procesa los ya terminados).
        let pending = collect_pending(&self.jobs);
        if pending.is_empty() {
            return;
        }

        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        self.running = true;
        self.cancel_flag.store(false, Ordering::SeqCst);

        for job in self.jobs.iter_mut() {
            if job.state == JobState::Queued {
                job.status = "En cola".to_string();
            }
        }

        // Empieza un registro nuevo para esta corrida (trunca el anterior).
        let _ = std::fs::File::create(&self.log_path);

        let out_dir = self.out_dir.clone();
        let worker = Worker {
            ffmpeg: self.ffmpeg.clone(),
            tx,
            cancel_flag: self.cancel_flag.clone(),
            current_child: self.current_child.clone(),
            log_path: self.log_path.clone(),
        };

        std::thread::spawn(move || {
            run_queue(worker, pending, target_mb, max_height, out_dir);
        });
    }

    /// Drena los mensajes del hilo de trabajo y actualiza el estado de los jobs.
    fn poll(&mut self) {
        let mut finished_all = false;
        if let Some(rx) = &self.rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    Msg::Progress { id, percent, phase } => {
                        if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
                            j.state = JobState::Processing;
                            j.progress = percent;
                            j.status = if percent <= 0.0 {
                                phase.to_string()
                            } else {
                                format!("{phase} · {}%", percent.round())
                            };
                        }
                    }
                    Msg::Done {
                        id,
                        output,
                        final_bytes,
                        warning,
                    } => {
                        if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
                            j.state = JobState::Done;
                            j.progress = 100.0;
                            let saved = j.orig_bytes.filter(|&o| o > 0).map(|o| {
                                (100.0 * (1.0 - final_bytes as f64 / o as f64)).round() as i64
                            });
                            // Base: cuánto se logró comprimir (siempre se muestra).
                            let base = match saved {
                                Some(s) if s > 0 => {
                                    format!("Listo · {s}% más liviano ({})", fmt_size(final_bytes))
                                }
                                _ => format!("Listo ({})", fmt_size(final_bytes)),
                            };
                            // Si hubo advertencia, se añade para que el usuario sepa
                            // que no se llegó al objetivo y por qué.
                            j.status = match &warning {
                                Some(w) => format!("⚠ {base} — {w}"),
                                None => base,
                            };
                            j.output = Some(output);
                        }
                    }
                    Msg::Error { id, message } => {
                        if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
                            j.state = JobState::Error;
                            j.status = format!("Error: {message}");
                        }
                    }
                    Msg::Canceled { id } => {
                        if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
                            j.state = JobState::Queued;
                            j.progress = 0.0;
                            j.status = "Cancelado".to_string();
                        }
                        finished_all = true;
                    }
                }
            }
        }
        if finished_all
            || self.jobs.iter().all(|j| j.state != JobState::Processing)
                && self.running
                && self.jobs.iter().all(|j| {
                    j.state == JobState::Done
                        || j.state == JobState::Error
                        || j.state == JobState::Queued && !self.cancel_flag.load(Ordering::SeqCst)
                })
        {
            // Si ya no queda nada "processing" y no hay pendientes en la cola activa, terminamos.
            let still_pending_active =
                self.running && self.jobs.iter().any(|j| j.state == JobState::Processing);
            if !still_pending_active {
                // Verifica si el hilo de trabajo sigue vivo revisando si el canal se cerró
                if let Some(rx) = &self.rx {
                    if rx.try_recv().is_err() {
                        // canal vacío; asumimos que terminó si no hay Processing
                        let any_processing =
                            self.jobs.iter().any(|j| j.state == JobState::Processing);
                        if !any_processing {
                            self.running = false;
                        }
                    }
                }
            }
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        self.ensure_logo(ctx);
        if self.running || self.update_in_progress() {
            ctx.request_repaint_after(Duration::from_millis(120));
        }

        // Drag & drop de archivos
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        for p in dropped {
            self.add_file(p);
        }

        let logo_id = self.logo.as_ref().map(|t| t.id());
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if let Some(id) = logo_id {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                        id,
                        egui::vec2(36.0, 36.0),
                    )));
                    ui.add_space(4.0);
                }
                ui.heading("R2D2 Compactor");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak(format!("v{}", update::current_version()));
                });
            });
            ui.label(
                "Reduce el peso de videos e imágenes de evidencia antes de subirlos. \
                 Arrastra los archivos, define el tamaño objetivo y comprime.",
            );
            if self.ffmpeg_missing {
                ui.colored_label(
                    egui::Color32::from_rgb(248, 113, 113),
                    "No se encontró FFmpeg. Descomprime el .zip completo: la app y la carpeta \
                     'ffmpeg' deben quedar juntas en el mismo lugar.",
                );
            }
            self.show_update_banner(ui);
            ui.separator();

            ui.horizontal_top(|ui| {
                // --- Tamaño objetivo ---
                ui.vertical(|ui| {
                    ui.label("🎯 Tamaño objetivo (MB)")
                        .on_hover_text("Peso máximo que tendrá cada archivo comprimido.");
                    ui.add(egui::TextEdit::singleline(&mut self.target_mb).desired_width(80.0));
                    ui.small("Menor número = más liviano,\npero algo menos de calidad.");
                });
                ui.add_space(20.0);

                // --- Resolución máxima ---
                ui.vertical(|ui| {
                    ui.label("📐 Resolución máxima")
                        .on_hover_text("Reduce las dimensiones para ahorrar peso. Nunca agranda.");
                    egui::ComboBox::from_id_source("max_height")
                        .width(180.0)
                        .selected_text(match self.max_height_idx {
                            0 => "1080p (Full HD)",
                            1 => "720p (HD)",
                            _ => "Original (sin cambiar)",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.max_height_idx,
                                0,
                                "1080p (Full HD) · recomendado",
                            );
                            ui.selectable_value(
                                &mut self.max_height_idx,
                                1,
                                "720p (HD) · más liviano",
                            );
                            ui.selectable_value(
                                &mut self.max_height_idx,
                                2,
                                "Original (sin cambiar tamaño)",
                            );
                        });
                    ui.small("Si la fuente ya es menor,\nse deja como está.");
                });
                ui.add_space(20.0);

                // --- Carpeta de salida ---
                ui.vertical(|ui| {
                    ui.label("📁 Carpeta de salida")
                        .on_hover_text("Dónde se guardan los archivos comprimidos.");
                    ui.horizontal(|ui| {
                        let text = self
                            .out_dir
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Misma carpeta del original".to_string());
                        ui.add(
                            egui::TextEdit::singleline(&mut text.clone())
                                .desired_width(220.0)
                                .interactive(false),
                        );
                        if ui.button("Elegir…").clicked() {
                            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                                self.out_dir = Some(dir);
                            }
                        }
                        if ui
                            .button("↺")
                            .on_hover_text("Volver a la carpeta del original")
                            .clicked()
                        {
                            self.out_dir = None;
                        }
                    });
                    ui.small("El original no se modifica;\nse crea una copia con sufijo «_comp».");
                });
            });
            ui.add_space(10.0);
        });

        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let done = self
                    .jobs
                    .iter()
                    .filter(|j| j.state == JobState::Done)
                    .count();
                let pending = self
                    .jobs
                    .iter()
                    .filter(|j| j.state == JobState::Queued)
                    .count();
                let summary = if self.jobs.is_empty() {
                    "Sin archivos en cola".to_string()
                } else {
                    format!(
                        "{} archivo(s) · {} listo(s) · {} en cola",
                        self.jobs.len(),
                        done,
                        pending
                    )
                };
                ui.label(summary);
                if self.running {
                    ui.spinner();
                    ui.label("Procesando…");
                }
                if self.log_path.exists()
                    && ui
                        .button("Ver registro")
                        .on_hover_text("Abre el registro técnico de la última compresión (FFmpeg)")
                        .clicked()
                {
                    open_file(&self.log_path);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_run = !self.running && pending > 0 && !self.ffmpeg_missing;
                    if ui
                        .add_enabled(can_run, egui::Button::new("Comprimir todo"))
                        .clicked()
                    {
                        self.start_run();
                    }
                    if ui
                        .add_enabled(self.running, egui::Button::new("Cancelar"))
                        .clicked()
                    {
                        self.cancel_flag.store(true, Ordering::SeqCst);
                        self.stop_current_child();
                    }
                });
            });
            ui.add_space(6.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let frame = egui::Frame::none()
                    .stroke(egui::Stroke::new(1.5, egui::Color32::from_gray(90)))
                    .rounding(12.0)
                    .inner_margin(egui::Margin::symmetric(20.0, 30.0));
                frame.show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("Arrastra aquí tus videos o imágenes").strong(),
                        );
                        ui.label("o usa el botón para elegirlos");
                        ui.small("Videos: MP4, MOV, AVI, MKV, M4V, WMV  ·  Imágenes: JPG, PNG, WEBP, BMP, TIFF");
                        ui.add_space(4.0);
                        if ui.button("Elegir archivos…").clicked() {
                            if let Some(files) = rfd::FileDialog::new()
                                .add_filter(
                                    "Videos e imágenes",
                                    &[
                                        "mp4", "mov", "avi", "mkv", "m4v", "wmv", "jpg", "jpeg",
                                        "png", "webp", "bmp", "tif", "tiff", "heic",
                                    ],
                                )
                                .pick_files()
                            {
                                for f in files {
                                    self.add_file(f);
                                }
                            }
                        }
                    });
                });

                ui.add_space(16.0);

                // Barra de acciones masivas (evita borrar archivo por archivo).
                let mut clear_done = false;
                let mut clear_all = false;
                if !self.jobs.is_empty() {
                    ui.horizontal(|ui| {
                        let has_done = self
                            .jobs
                            .iter()
                            .any(|j| matches!(j.state, JobState::Done | JobState::Error));
                        let removable =
                            self.jobs.iter().any(|j| j.state != JobState::Processing);
                        if ui
                            .add_enabled(has_done, egui::Button::new("🧹 Quitar terminados"))
                            .on_hover_text("Quita de la lista los que ya se comprimieron o fallaron")
                            .clicked()
                        {
                            clear_done = true;
                        }
                        if ui
                            .add_enabled(removable, egui::Button::new("🗑 Quitar todos"))
                            .on_hover_text("Vacía la lista (los que se están comprimiendo se conservan)")
                            .clicked()
                        {
                            clear_all = true;
                        }
                    });
                    ui.add_space(8.0);
                }

                let mut to_remove: Option<u64> = None;
                let mut to_open: Option<PathBuf> = None;

                for job in &self.jobs {
                    let frame = egui::Frame::none()
                        .fill(egui::Color32::from_gray(30))
                        .rounding(10.0)
                        .inner_margin(egui::Margin::symmetric(14.0, 12.0));
                    frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let icon = match job.kind {
                                MediaKind::Video => "🎬",
                                MediaKind::Image => "🖼",
                            };
                            ui.label(egui::RichText::new(format!("{icon} {}", job.name)).strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if job.state != JobState::Processing
                                        && ui.small_button("✕").clicked()
                                    {
                                        to_remove = Some(job.id);
                                    }
                                },
                            );
                        });
                        ui.horizontal(|ui| {
                            let orig = job
                                .orig_bytes
                                .map(fmt_size)
                                .unwrap_or_else(|| "…".to_string());
                            ui.label(orig);
                            ui.label("→");
                            ui.label(format!("objetivo {} MB", self.target_mb));
                        });
                        ui.add(egui::ProgressBar::new(job.progress / 100.0).desired_height(6.0));
                        ui.horizontal(|ui| {
                            if job.state == JobState::Processing {
                                ui.spinner();
                            }
                            ui.label(&job.status);
                            if job.state == JobState::Done {
                                if let Some(out) = &job.output {
                                    if ui.link("Ver archivo").clicked() {
                                        to_open = Some(out.clone());
                                    }
                                }
                            }
                        });
                    });
                    ui.add_space(10.0);
                }

                if clear_done {
                    self.jobs
                        .retain(|j| !matches!(j.state, JobState::Done | JobState::Error));
                }
                if clear_all {
                    // Conserva solo los que se están comprimiendo en este momento.
                    self.jobs.retain(|j| j.state == JobState::Processing);
                }
                if let Some(id) = to_remove {
                    self.jobs.retain(|j| j.id != id);
                }
                if let Some(path) = to_open {
                    open_containing_folder(&path);
                }
            });
        });
    }

    /// Se ejecuta al cerrar la ventana: detiene cualquier FFmpeg en curso para
    /// que no quede un proceso huérfano corriendo en segundo plano.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        self.stop_current_child();
    }
}
