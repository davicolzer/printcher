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
    let dest_path = super::temp_capture_path()?;
    // Não usamos std::fs::copy: ela também copia os bits de permissão da
    // origem, e o arquivo temporário do portal (montado via document portal
    // do Flatpak) costuma ser somente leitura -- isso deixaria a captura
    // final sem permissão de escrita, quebrando o Salvar do editor depois.
    // Ler e escrever de novo usa o modo padrão do processo (respeitando o
    // umask), sempre gravável.
    let bytes = std::fs::read(&source_path)?;
    std::fs::write(&dest_path, bytes)?;

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
