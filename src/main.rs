//! Compresor de videos de evidencia para Aforo (two-pass H.264 vía FFmpeg).
//!
//! Punto de entrada: configura la ventana nativa y arranca la app egui.

// En release en Windows, oculta la ventana de consola detrás de la GUI.
// En debug la deja para ver panics/salida durante el desarrollo.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod ffmpeg;
mod install;
mod model;
mod queue;
mod update;
mod util;

use app::App;
use eframe::egui;
use std::sync::Arc;

/// Logo de la app, embebido en el binario. Se muestra como icono de la ventana,
/// en la barra de tareas y en el encabezado. Para cambiarlo, reemplaza
/// `assets/icon.png` por un PNG cuadrado (ideal 256×256, fondo transparente).
pub(crate) const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([900.0, 720.0])
        .with_min_inner_size([740.0, 600.0]);

    // Si el PNG del icono es válido, se aplica a la ventana; si no, se ignora.
    if let Ok(icon) = eframe::icon_data::from_png_bytes(ICON_PNG) {
        viewport = viewport.with_icon(Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "R2D2 Compactor",
        options,
        Box::new(|_cc| Box::new(App::new())),
    )
}
