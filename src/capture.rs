mod wayland;
mod x11;

use std::path::PathBuf;

use chrono::Local;

/// Captura a tela cheia, escolhendo o backend certo pra sessão atual
/// (Wayland via xdg-desktop-portal, X11 via conexão direta) e salva o
/// resultado num arquivo temporário — só o resultado final da edição
/// (depois de Salvar) vai pra `~/Pictures/printcher/`. Assim, cancelar a
/// edição (ou simplesmente fechar a janela) não deixa a captura da tela
/// cheia perdida em `~/Pictures/printcher/`.
pub async fn capture_fullscreen() -> anyhow::Result<PathBuf> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        wayland::capture_fullscreen().await
    } else {
        x11::capture_fullscreen()
    }
}

/// Gera um caminho novo em `~/Pictures/printcher/printcher_<timestamp>.png`,
/// criando o diretório se necessário. Usado só pelo Salvar do editor — é o
/// único jeito de um arquivo acabar em `~/Pictures/printcher/`.
pub(crate) fn dest_path() -> anyhow::Result<PathBuf> {
    let dest_dir = dirs::picture_dir()
        .unwrap_or_else(|| dirs::home_dir().expect("home directory not found"))
        .join("printcher");
    std::fs::create_dir_all(&dest_dir)?;

    let file_name = format!("printcher_{}.png", Local::now().format("%Y%m%d_%H%M%S"));
    Ok(dest_dir.join(file_name))
}

/// Caminho temporário pra guardar a captura crua enquanto o usuário edita —
/// fora de `~/Pictures/printcher/` de propósito (ver `capture_fullscreen`).
pub(crate) fn temp_capture_path() -> anyhow::Result<PathBuf> {
    let dir = std::env::temp_dir().join("printcher");
    std::fs::create_dir_all(&dir)?;

    let file_name = format!("capture_{}.png", Local::now().format("%Y%m%d_%H%M%S%.f"));
    Ok(dir.join(file_name))
}
