use std::path::PathBuf;

use ashpd::desktop::screenshot::Screenshot;
use chrono::Local;
use percent_encoding::percent_decode_str;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dest_path = capture_fullscreen().await?;
    println!("Screenshot salvo em: {}", dest_path.display());
    Ok(())
}

/// Captura a tela cheia via xdg-desktop-portal e salva uma cópia
/// em `~/Pictures/printcher/`, retornando o caminho salvo.
async fn capture_fullscreen() -> anyhow::Result<PathBuf> {
    let response = Screenshot::request()
        .interactive(false)
        .modal(true)
        .send()
        .await?
        .response()?;

    let source_path = uri_to_path(response.uri().as_str())?;

    let dest_dir = dirs::picture_dir()
        .unwrap_or_else(|| dirs::home_dir().expect("home directory not found"))
        .join("printcher");
    std::fs::create_dir_all(&dest_dir)?;

    let file_name = format!("printcher_{}.png", Local::now().format("%Y%m%d_%H%M%S"));
    let dest_path = dest_dir.join(file_name);

    std::fs::copy(&source_path, &dest_path)?;

    Ok(dest_path)
}

/// Converte uma URI `file://...` retornada pelo portal em um caminho local,
/// decodificando eventuais caracteres percent-encoded.
fn uri_to_path(uri: &str) -> anyhow::Result<PathBuf> {
    let raw = uri
        .strip_prefix("file://")
        .ok_or_else(|| anyhow::anyhow!("URI inesperada do portal: {uri}"))?;
    let decoded = percent_decode_str(raw).decode_utf8()?;
    Ok(PathBuf::from(decoded.into_owned()))
}
