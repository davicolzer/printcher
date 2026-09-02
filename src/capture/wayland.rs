use std::path::PathBuf;

use ashpd::desktop::screenshot::Screenshot;
use percent_encoding::percent_decode_str;

/// Captura a tela cheia via xdg-desktop-portal (`org.freedesktop.portal.Screenshot`)
/// e copia o resultado pro destino padrão do printcher.
pub async fn capture_fullscreen() -> anyhow::Result<PathBuf> {
    let response = Screenshot::request()
        .interactive(false)
        .modal(true)
        .send()
        .await?
        .response()?;

    let source_path = uri_to_path(response.uri().as_str())?;
    let dest_path = super::dest_path()?;
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
