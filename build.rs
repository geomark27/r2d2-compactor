//! Script de compilación: incrusta el icono del ejecutable en Windows.
//!
//! Convierte `assets/icon.png` a un `.ico` multi-resolución y lo incrusta como
//! recurso del `.exe`, para que el logo aparezca en el Explorador de Windows
//! (no solo en la ventana/barra de tareas en tiempo de ejecución).
//!
//! Solo actúa al compilar para Windows; en Linux/Mac no hace nada.

use std::path::PathBuf;

fn main() {
    // Regenerar si cambia el logo.
    println!("cargo:rerun-if-changed=assets/icon.png");

    // Solo incrustamos el recurso cuando el objetivo es Windows.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR no definido"));
    let ico_path = out_dir.join("icon.ico");

    // 1. Cargar el PNG y generar un .ico con varios tamaños (16..256) para que
    //    el Explorador use el más nítido según el contexto.
    let src = image::open("assets/icon.png")
        .expect("no se pudo abrir assets/icon.png")
        .to_rgba8();
    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16u32, 32, 48, 64, 128, 256] {
        let resized =
            image::imageops::resize(&src, size, size, image::imageops::FilterType::Lanczos3);
        let img = ico::IconImage::from_rgba_data(size, size, resized.into_raw());
        dir.add_entry(ico::IconDirEntry::encode(&img).expect("no se pudo codificar el icono"));
    }
    let file = std::fs::File::create(&ico_path).expect("no se pudo crear icon.ico");
    dir.write(file).expect("no se pudo escribir icon.ico");

    // 2. Incrustar el .ico como recurso del ejecutable.
    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_str().expect("ruta de icono no válida"));

    // Al cross-compilar desde Linux (target *-gnu) hay que usar el windres de
    // mingw; en un build nativo con MSVC winresource ya lo resuelve solo.
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.ends_with("-gnu") {
        res.set_windres_path("x86_64-w64-mingw32-windres");
    }

    res.compile()
        .expect("no se pudo incrustar el icono en el .exe");
}
