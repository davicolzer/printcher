mod wayland;
mod x11;

use std::path::PathBuf;

use chrono::{DateTime, Local};

use crate::config::Config;

/// Captura a tela cheia, escolhendo o backend certo pra sessão atual
/// (Wayland via xdg-desktop-portal, X11 via conexão direta) e salva o
/// resultado num arquivo temporário — só o resultado final da edição
/// (depois de Salvar) vai pra pasta de destino configurada. Assim, cancelar
/// a edição (ou simplesmente fechar a janela) não deixa a captura da tela
/// cheia perdida por lá.
pub async fn capture_fullscreen() -> anyhow::Result<PathBuf> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        wayland::capture_fullscreen().await
    } else {
        x11::capture_fullscreen()
    }
}

/// Pasta de destino das capturas: a escolhida pelo usuário
/// (`cfg.folder`), ou por padrão `~/Imagens/printcher` (acompanha se a
/// pasta de Imagens do sistema mudar de lugar, ao contrário de gravar um
/// caminho fixo desde o início).
pub(crate) fn dest_dir(cfg: &Config) -> PathBuf {
    cfg.folder.clone().unwrap_or_else(|| {
        dirs::picture_dir()
            .unwrap_or_else(|| dirs::home_dir().expect("home directory not found"))
            .join("printcher")
    })
}

/// Gera um caminho novo na pasta de destino configurada, com o nome
/// seguindo o padrão e formato escolhidos pelo usuário (`cfg.pattern`,
/// `cfg.format`), criando o diretório se necessário. Usado só pelo Salvar
/// do editor — é o único jeito de um arquivo acabar lá.
pub(crate) fn dest_path(cfg: &Config) -> anyhow::Result<PathBuf> {
    let dest_dir = dest_dir(cfg);
    std::fs::create_dir_all(&dest_dir)?;

    // Contador `%n`: conta os arquivos já existentes em vez de guardar um
    // número à parte em `Config` -- mais simples, sem estado extra pra
    // sincronizar, mesmo resultado prático (cresce a cada captura salva).
    let counter = std::fs::read_dir(&dest_dir).map(|entries| entries.count() as u32).unwrap_or(0) + 1;
    let file_name = format!("{}.{}", expand_pattern(&cfg.pattern, Local::now(), counter), cfg.format.extension());
    Ok(dest_dir.join(file_name))
}

/// Substitui os códigos do padrão de nome de arquivo: `%Y %m %d %H %M %S`
/// (data/hora) e `%n` (contador, com zero à esquerda até 3 dígitos). Texto
/// fora dos códigos é mantido como está.
pub(crate) fn expand_pattern(pattern: &str, now: DateTime<Local>, counter: u32) -> String {
    pattern
        .replace("%Y", &now.format("%Y").to_string())
        .replace("%m", &now.format("%m").to_string())
        .replace("%d", &now.format("%d").to_string())
        .replace("%H", &now.format("%H").to_string())
        .replace("%M", &now.format("%M").to_string())
        .replace("%S", &now.format("%S").to_string())
        .replace("%n", &format!("{counter:03}"))
}

/// Caminho temporário pra guardar a captura crua enquanto o usuário edita —
/// fora de `~/Pictures/printcher/` de propósito (ver `capture_fullscreen`).
pub(crate) fn temp_capture_path() -> anyhow::Result<PathBuf> {
    let dir = std::env::temp_dir().join("printcher");
    std::fs::create_dir_all(&dir)?;

    let file_name = format!("capture_{}.png", Local::now().format("%Y%m%d_%H%M%S%.f"));
    Ok(dir.join(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_time() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 3, 7, 9, 5, 3).unwrap()
    }

    #[test]
    fn expand_pattern_substitutes_every_date_code() {
        let out = expand_pattern("printcher_%Y-%m-%d_%H-%M-%S", sample_time(), 1);
        assert_eq!(out, "printcher_2026-03-07_09-05-03");
    }

    #[test]
    fn expand_pattern_pads_the_counter_to_three_digits() {
        assert_eq!(expand_pattern("captura_%n", sample_time(), 7), "captura_007");
        assert_eq!(expand_pattern("captura_%n", sample_time(), 42), "captura_042");
        assert_eq!(expand_pattern("captura_%n", sample_time(), 1234), "captura_1234");
    }

    #[test]
    fn expand_pattern_keeps_text_outside_codes_untouched() {
        assert_eq!(expand_pattern("sem códigos aqui", sample_time(), 1), "sem códigos aqui");
    }

    #[test]
    fn dest_dir_uses_configured_folder_when_set() {
        let cfg = Config {
            folder: Some(PathBuf::from("/tmp/printcher-custom-dest")),
            ..Default::default()
        };
        assert_eq!(dest_dir(&cfg), PathBuf::from("/tmp/printcher-custom-dest"));
    }

    #[test]
    fn dest_path_names_the_file_with_the_configured_pattern_and_format() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = Config {
            folder: Some(tmp.path().to_path_buf()),
            pattern: "shot_%n".to_string(),
            format: crate::config::ImageFormat::WebP,
            ..Default::default()
        };

        let path = dest_path(&cfg).unwrap();
        assert_eq!(path, tmp.path().join("shot_001.webp"));
    }
}
