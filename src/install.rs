//! Integración con el sistema operativo.
//!
//! Crea un acceso directo en el menú Inicio de Windows para que la app aparezca
//! al escribir su nombre en el buscador, sin que el usuario tenga que hacerlo.

/// Asegura que exista el acceso directo en el menú Inicio (Windows). Si ya está,
/// no hace nada. Fuera de Windows es un no-op. Es "best-effort": cualquier fallo
/// se ignora en silencio (nunca debe impedir que la app arranque).
#[cfg(windows)]
pub fn ensure_start_menu_shortcut() {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Ok(appdata) = std::env::var("APPDATA") else {
        return;
    };

    let lnk = format!("{appdata}\\Microsoft\\Windows\\Start Menu\\Programs\\R2D2 Compactor.lnk");
    if std::path::Path::new(&lnk).exists() {
        return; // ya existe; no repetir el trabajo en cada arranque
    }

    // Escapa comillas simples para incrustar rutas en el script de PowerShell.
    let esc = |s: String| s.replace('\'', "''");
    let exe_str = esc(exe.to_string_lossy().into_owned());
    let dir = esc(exe
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default());
    let lnk_ps = esc(lnk);

    let script = format!(
        "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{lnk_ps}');\
         $s.TargetPath='{exe_str}';$s.WorkingDirectory='{dir}';$s.Save()"
    );

    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-WindowStyle",
        "Hidden",
        "-Command",
        &script,
    ]);
    cmd.creation_flags(CREATE_NO_WINDOW);
    let _ = cmd.status();
}

#[cfg(not(windows))]
pub fn ensure_start_menu_shortcut() {}
