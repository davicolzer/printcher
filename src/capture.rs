mod wayland;
mod x11;

use std::path::PathBuf;

use chrono::Local;

/// Captura a tela cheia, escolhendo o backend certo pra sessão atual
/// (Wayland via xdg-desktop-portal, X11 via conexão direta) e salva o
/// resultado em `~/Pictures/printcher/`.
pub async fn capture_fullscreen() -> anyhow::Result<PathBuf> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        wayland::capture_fullscreen().await
    } else {
        x11::capture_fullscreen()
    }
}

/// Gera um caminho novo em `~/Pictures/printcher/printcher_<timestamp>.png`,
/// criando o diretório se necessário.
pub(crate) fn dest_path() -> anyhow::Result<PathBuf> {
    let dest_dir = dirs::picture_dir()
        .unwrap_or_else(|| dirs::home_dir().expect("home directory not found"))
        .join("printcher");
    std::fs::create_dir_all(&dest_dir)?;

    let file_name = format!("printcher_{}.png", Local::now().format("%Y%m%d_%H%M%S"));
    Ok(dest_dir.join(file_name))
}
