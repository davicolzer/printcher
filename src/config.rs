use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Tema da interface -- afeta a janela de Configurações (a barra flutuante
/// do editor usa cores fixas da marca, não muda com isso).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

/// Formato de imagem pra salvar a captura -- afeta tanto a extensão do
/// arquivo quanto o codificador usado (`editor::render::save_surface_as`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ImageFormat {
    #[default]
    Png,
    Jpg,
    WebP,
}

impl ImageFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpg => "jpg",
            ImageFormat::WebP => "webp",
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_pattern() -> String {
    "printcher_%Y-%m-%d_%H-%M-%S".to_string()
}

/// Configurações persistentes do printcher. O atalho de captura em si NÃO
/// mora aqui — quem guarda isso é o portal/compositor, via
/// `GlobalShortcuts::configure_shortcuts`. Este arquivo é só pra
/// configurações que o próprio printcher precisa lembrar entre execuções.
///
/// Novas opções (pasta de destino, cor padrão de anotação, etc.) entram
/// aqui como novos campos com `#[serde(default)]`, pra não quebrar
/// configs antigos.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub start_on_login: bool,
    /// `#[serde(default = "default_true")]` em vez de depender do
    /// `#[serde(default)]` do struct (que usaria `bool::default()` =
    /// `false`): configs salvos antes desse campo existir não têm ele no
    /// TOML, e sem isso o ícone da bandeja sumiria sozinho pra quem já
    /// tinha o printcher instalado, na primeira vez que abrisse depois da
    /// atualização.
    #[serde(default = "default_true")]
    pub tray_enabled: bool,
    pub theme: Theme,
    /// Só persiste a preferência por enquanto -- o efeito visual de
    /// trocar pra fonte OpenDyslexic ainda depende de empacotar a fonte no
    /// Flatpak, que é trabalho de uma fase futura.
    pub dyslexia_font: bool,
    /// `None` = usa o padrão calculado (`dirs::picture_dir()/printcher`) --
    /// só grava um caminho fixo aqui quando o usuário escolhe outra pasta
    /// explicitamente, pra continuar acompanhando se a pasta de Imagens do
    /// sistema mudar de lugar.
    pub folder: Option<PathBuf>,
    pub format: ImageFormat,
    #[serde(default = "default_pattern")]
    pub pattern: String,
    /// Copiar a captura pra área de transferência automaticamente ao
    /// salvar, além de gravar o arquivo.
    #[serde(default = "default_true")]
    pub auto_copy: bool,
}

/// Carrega a configuração salva, ou os valores padrão se não existir/estiver
/// corrompida.
pub fn load() -> Config {
    std::fs::read_to_string(config_path())
        .ok()
        .and_then(|contents| toml::from_str(&contents).ok())
        .unwrap_or_default()
}

/// Como [`load`], mas na primeira execução (nenhum arquivo de config ainda)
/// já liga "iniciar com o sistema" por padrão e registra o autostart.
/// Chamar só uma vez, ao virar daemon de verdade — não a cada tentativa de
/// cliente. Retorna também se era a primeira execução, pra quem chamou
/// poder dar as boas-vindas (ex: banner na tela de configurações).
pub fn load_or_init() -> (Config, bool) {
    if config_path().exists() {
        return (load(), false);
    }

    let cfg = Config {
        start_on_login: true,
        tray_enabled: true,
        pattern: default_pattern(),
        auto_copy: true,
        ..Default::default()
    };
    if let Err(e) = save(&cfg) {
        eprintln!("Erro ao salvar configuração inicial: {e}");
    }
    if let Err(e) = crate::autostart::install() {
        eprintln!("Erro ao ligar autostart na primeira execução: {e}");
    }
    (cfg, true)
}

pub fn save(config: &Config) -> anyhow::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, toml::to_string_pretty(config)?)?;
    Ok(())
}

fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

fn config_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| dirs::home_dir().expect("home directory not found"));
    base.join("printcher")
}

/// Remove o diretório de configuração inteiro (`~/.config/printcher/`).
/// Não mexe nos screenshots salvos (`~/Pictures/printcher/`) — são
/// conteúdo do usuário, não rastro do app.
pub fn remove_all() -> anyhow::Result<()> {
    let dir = config_dir();
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_autostart_off() {
        assert!(!Config::default().start_on_login);
    }

    #[test]
    fn load_without_file_returns_default() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::testutil::set_xdg_config_home(tmp.path());

        let cfg = load();
        assert!(!cfg.start_on_login);
    }

    #[test]
    fn save_then_load_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::testutil::set_xdg_config_home(tmp.path());

        save(&Config {
            start_on_login: true,
            ..Default::default()
        })
        .unwrap();

        assert!(load().start_on_login);
        assert!(tmp.path().join("printcher/config.toml").exists());
    }

    #[test]
    fn tray_enabled_defaults_to_true_when_missing_from_an_existing_config_file() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::testutil::set_xdg_config_home(tmp.path());

        // Simula um config.toml salvo antes do campo `tray_enabled` existir.
        let dir = tmp.path().join("printcher");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.toml"), "start_on_login = true\n").unwrap();

        assert!(load().tray_enabled, "configs antigos não devem perder o ícone da bandeja sozinhos");
    }

    #[test]
    fn theme_defaults_to_system() {
        assert_eq!(Config::default().theme, Theme::System);
    }

    #[test]
    fn load_or_init_enables_tray_by_default() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::testutil::set_xdg_config_home(tmp.path());

        let (cfg, _) = load_or_init();
        assert!(cfg.tray_enabled);
    }

    #[test]
    fn load_or_init_only_runs_first_time() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::testutil::set_xdg_config_home(tmp.path());

        let (cfg, first) = load_or_init();
        assert!(first);
        assert!(cfg.start_on_login);

        let (_, first_again) = load_or_init();
        assert!(!first_again);
    }

    #[test]
    fn remove_all_deletes_the_config_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::testutil::set_xdg_config_home(tmp.path());

        save(&Config::default()).unwrap();
        assert!(tmp.path().join("printcher").exists());

        remove_all().unwrap();
        assert!(!tmp.path().join("printcher").exists());
    }

    #[test]
    fn remove_all_is_a_noop_when_nothing_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::testutil::set_xdg_config_home(tmp.path());

        remove_all().unwrap();
    }
}
