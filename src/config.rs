use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
/// cliente.
pub fn load_or_init() -> Config {
    if config_path().exists() {
        return load();
    }

    let cfg = Config {
        start_on_login: true,
    };
    if let Err(e) = save(&cfg) {
        eprintln!("Erro ao salvar configuração inicial: {e}");
    }
    if let Err(e) = crate::autostart::install() {
        eprintln!("Erro ao ligar autostart na primeira execução: {e}");
    }
    cfg
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
    let config_dir = dirs::config_dir().unwrap_or_else(|| dirs::home_dir().expect("home directory not found"));
    config_dir.join("printcher").join("config.toml")
}
