//! Aplicación egui: estado de la UI, cola de trabajos y renderizado.

use eframe::egui;
use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::ffmpeg::{probe_duration, resolve_tool, which_in_path, Worker};
use crate::model::{Job, JobState, Msg};
use crate::queue::run_queue;
use crate::util::{fmt_size, open_containing_folder};

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
}

impl App {
    pub fn new() -> Self {
        let ffmpeg = resolve_tool("ffmpeg.exe", "ffmpeg");
        let ffprobe = resolve_tool("ffprobe.exe", "ffprobe");
        let missing = !ffmpeg.exists() && which_in_path(&ffmpeg).is_none();
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
        }
    }

    /// Añade un video a la cola, evitando duplicados y leyendo su metadata.
    fn add_file(&mut self, path: PathBuf) {
        if self.jobs.iter().any(|j| j.input == path) {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("video")
            .to_string();
        let orig_bytes = std::fs::metadata(&path).ok().map(|m| m.len());
        let duration = probe_duration(&self.ffprobe, &path).ok();
        self.jobs.push(Job {
            id,
            input: path,
            name,
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
        let pending: Vec<(u64, PathBuf, f64)> = self
            .jobs
            .iter()
            .filter(|j| j.state == JobState::Queued)
            .map(|j| (j.id, j.input.clone(), j.duration.unwrap_or(0.0)))
            .collect();
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

        let out_dir = self.out_dir.clone();
        let worker = Worker {
            ffmpeg: self.ffmpeg.clone(),
            tx,
            cancel_flag: self.cancel_flag.clone(),
            current_child: self.current_child.clone(),
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
                            j.status = format!("{phase} · {}%", percent.round());
                        }
                    }
                    Msg::Done {
                        id,
                        final_bytes,
                        warning,
                    } => {
                        if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
                            j.state = JobState::Done;
                            j.progress = 100.0;
                            let saved = j.orig_bytes.filter(|&o| o > 0).map(|o| {
                                (100.0 * (1.0 - final_bytes as f64 / o as f64)).round() as i64
                            });
                            j.status = match (saved, &warning) {
                                (Some(s), None) if s > 0 => {
                                    format!("Listo · {s}% más liviano ({})", fmt_size(final_bytes))
                                }
                                (_, Some(w)) => w.clone(),
                                _ => format!("Listo ({})", fmt_size(final_bytes)),
                            };
                            let dir = self.out_dir.clone().unwrap_or_else(|| {
                                j.input
                                    .parent()
                                    .map(|p| p.to_path_buf())
                                    .unwrap_or_else(|| PathBuf::from("."))
                            });
                            let stem = j
                                .input
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("video");
                            j.output = Some(dir.join(format!("{stem}_comp.mp4")));
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
        if self.running {
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

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(10.0);
            ui.heading("Compresor de Evidencias");
            ui.label("Reduce videos pesados a un tamaño objetivo conservando la calidad. FFmpeg embebido.");
            if self.ffmpeg_missing {
                ui.colored_label(
                    egui::Color32::from_rgb(248, 113, 113),
                    "No se encontró ffmpeg. Coloca ffmpeg.exe/ffprobe.exe en la carpeta 'ffmpeg' junto al programa.",
                );
            }
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label("Tamaño objetivo (MB)");
                    ui.add(egui::TextEdit::singleline(&mut self.target_mb).desired_width(80.0));
                });
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.label("Resolución máxima");
                    egui::ComboBox::from_id_source("max_height")
                        .selected_text(match self.max_height_idx {
                            0 => "1080p (Full HD)",
                            1 => "720p (HD)",
                            _ => "Original (no escalar)",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.max_height_idx, 0, "1080p (Full HD)");
                            ui.selectable_value(&mut self.max_height_idx, 1, "720p (HD)");
                            ui.selectable_value(&mut self.max_height_idx, 2, "Original (no escalar)");
                        });
                });
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.label("Carpeta de salida");
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
                        if ui.button("↺").clicked() {
                            self.out_dir = None;
                        }
                    });
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
                        if let Ok(mut c) = self.current_child.lock() {
                            if let Some(child) = c.as_mut() {
                                let _ = child.kill();
                            }
                        }
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
                        ui.label(egui::RichText::new("Arrastra los videos aquí").strong());
                        ui.label("o usa el botón para elegirlos — MP4, MOV, AVI, MKV");
                        if ui.button("Elegir videos…").clicked() {
                            if let Some(files) = rfd::FileDialog::new()
                                .add_filter("Videos", &["mp4", "mov", "avi", "mkv", "m4v", "wmv"])
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

                let mut to_remove: Option<u64> = None;
                let mut to_open: Option<PathBuf> = None;

                for job in &self.jobs {
                    let frame = egui::Frame::none()
                        .fill(egui::Color32::from_gray(30))
                        .rounding(10.0)
                        .inner_margin(egui::Margin::symmetric(14.0, 12.0));
                    frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&job.name).strong());
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

                if let Some(id) = to_remove {
                    self.jobs.retain(|j| j.id != id);
                }
                if let Some(path) = to_open {
                    open_containing_folder(&path);
                }
            });
        });
    }
}
