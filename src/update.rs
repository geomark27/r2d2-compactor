//! Auto-actualización desde GitHub Releases.
//!
//! Replica el mecanismo del CLI `gtt`: consulta el release más reciente, compara
//! versiones, descarga el binario de la plataforma junto con su `checksums.txt`,
//! verifica el SHA-256 y reemplaza el ejecutable en uso.

use std::io::Read;
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Repositorio de GitHub donde se publican los releases.
const REPO: &str = "geomark27/r2d2-compactor";

/// Estado de la comprobación/aplicación de actualizaciones, compartido con la UI.
#[derive(Clone, Debug)]
pub enum UpdateStatus {
    /// Comprobando en segundo plano si hay una versión nueva.
    Checking,
    /// No hay nada nuevo (o la comprobación falló silenciosamente al arrancar).
    UpToDate,
    /// Hay una versión nueva disponible (tag, p. ej. `v1.0.1`).
    Available(String),
    /// Descargando y aplicando la actualización.
    Downloading,
    /// Actualización aplicada; requiere reiniciar la app.
    Updated(String),
    /// Falló una actualización iniciada por el usuario.
    Error(String),
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
}

/// Versión compilada, leída de `Cargo.toml` (`CARGO_PKG_VERSION`), sin prefijo `v`.
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Nombre del asset publicado en el release, según la plataforma.
fn asset_name() -> &'static str {
    if cfg!(windows) {
        "r2d2-compactor-windows-amd64.exe"
    } else {
        "r2d2-compactor-linux-amd64"
    }
}

/// Consulta el release más reciente en GitHub y devuelve su tag si es más nuevo
/// que la versión actual. `None` significa que ya está al día.
pub fn check_latest() -> Result<Option<String>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = ureq::get(&url)
        .set("User-Agent", "r2d2-compactor")
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(10))
        .call()
        .map_err(|e| format!("no se pudo consultar GitHub: {e}"))?;

    let release: GhRelease = resp
        .into_json()
        .map_err(|e| format!("respuesta inválida de GitHub: {e}"))?;

    if release.tag_name.is_empty() || !is_newer(&release.tag_name, current_version()) {
        return Ok(None);
    }
    Ok(Some(release.tag_name))
}

/// Devuelve `true` solo si `candidate` es estrictamente mayor que `base`.
/// Ambos en formato `MAJOR.MINOR.PATCH` (con `v` opcional); si no parsean, `false`.
fn is_newer(candidate: &str, base: &str) -> bool {
    fn parse(v: &str) -> Option<(u32, u32, u32)> {
        let v = v.trim().trim_start_matches('v');
        let mut parts = v.splitn(3, '.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        Some((major, minor, patch))
    }
    match (parse(candidate), parse(base)) {
        (Some(c), Some(b)) => c > b,
        _ => false,
    }
}

/// Descarga `checksums.txt` del release y devuelve el hash SHA-256 del asset dado.
fn fetch_expected_hash(version: &str, asset: &str) -> Result<String, String> {
    let url = format!("https://github.com/{REPO}/releases/download/{version}/checksums.txt");
    let body = ureq::get(&url)
        .set("User-Agent", "r2d2-compactor")
        .timeout(Duration::from_secs(15))
        .call()
        .map_err(|e| format!("no se pudo descargar checksums.txt: {e}"))?
        .into_string()
        .map_err(|e| format!("checksums.txt ilegible: {e}"))?;

    for line in body.lines() {
        let mut fields = line.split_whitespace();
        if let (Some(hash), Some(name)) = (fields.next(), fields.next()) {
            if name == asset {
                return Ok(hash.to_string());
            }
        }
    }
    Err(format!("no se encontró el checksum de {asset}"))
}

/// Descarga la versión indicada, verifica su integridad y reemplaza el binario
/// en ejecución. Requiere reiniciar la app para que surta efecto.
pub fn self_update(version: &str) -> Result<(), String> {
    let asset = asset_name();
    let expected = fetch_expected_hash(version, asset)?;

    let url = format!("https://github.com/{REPO}/releases/download/{version}/{asset}");
    let resp = ureq::get(&url)
        .set("User-Agent", "r2d2-compactor")
        .timeout(Duration::from_secs(120))
        .call()
        .map_err(|e| format!("error descargando el binario: {e}"))?;

    let mut data = Vec::new();
    resp.into_reader()
        .read_to_end(&mut data)
        .map_err(|e| format!("error leyendo el binario: {e}"))?;

    let actual = hex::encode(Sha256::digest(&data));
    if actual != expected {
        return Err(
            "verificación de integridad fallida: el hash no coincide con el publicado".into(),
        );
    }

    // Escribe el binario nuevo a un temporal y deja que `self-replace` haga el
    // intercambio atómico (maneja el caso de Windows donde el .exe está en uso).
    let tmp = std::env::temp_dir().join("r2d2-compactor-update.bin");
    std::fs::write(&tmp, &data).map_err(|e| format!("no se pudo escribir el temporal: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755));
    }

    self_replace::self_replace(&tmp)
        .map_err(|e| format!("no se pudo reemplazar el binario: {e}"))?;
    let _ = std::fs::remove_file(&tmp);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn detecta_versiones_mas_nuevas() {
        assert!(is_newer("v1.0.1", "1.0.0"));
        assert!(is_newer("v1.1.0", "1.0.9"));
        assert!(is_newer("v2.0.0", "1.9.9"));
    }

    #[test]
    fn ignora_iguales_o_anteriores() {
        assert!(!is_newer("v1.0.0", "1.0.0"));
        assert!(!is_newer("v1.0.0", "1.0.1"));
        assert!(!is_newer("v1.2.0", "1.3.0"));
    }

    #[test]
    fn formatos_invalidos_no_actualizan() {
        assert!(!is_newer("latest", "1.0.0"));
        assert!(!is_newer("v1.0", "1.0.0"));
        assert!(!is_newer("", "1.0.0"));
    }
}
