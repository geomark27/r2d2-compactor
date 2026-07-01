//! Compresor de videos de evidencia para Aforo (two-pass H.264 vía FFmpeg).
//!
//! Punto de entrada: configura la ventana nativa y arranca la app egui.

mod app;
mod ffmpeg;
mod model;
mod queue;
mod util;

use app::App;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 720.0])
            .with_min_inner_size([740.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Compresor de Evidencias",
        options,
        Box::new(|_cc| Box::new(App::new())),
    )
}
