#![cfg(test)]
//! Helpers compartilhados entre os módulos de teste. Variáveis de ambiente
//! são estado global do processo, então testes que mexem nelas (pra isolar
//! `dirs::config_dir()`/`dirs::data_dir()` num diretório temporário) precisam
//! rodar um de cada vez -- [`EnvVarGuard`] segura um lock global enquanto
//! existir, e desfaz a mudança ao sair de escopo.

use std::ffi::OsString;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};

fn lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub struct EnvVarGuard {
    _lock: MutexGuard<'static, ()>,
    key: &'static str,
    previous: Option<OsString>,
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(v) => unsafe { std::env::set_var(self.key, v) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

fn set_env(key: &'static str, value: &Path) -> EnvVarGuard {
    let guard_lock = lock();
    let previous = std::env::var_os(key);
    unsafe { std::env::set_var(key, value) };
    EnvVarGuard {
        _lock: guard_lock,
        key,
        previous,
    }
}

/// Aponta `dirs::config_dir()` pra um diretório temporário enquanto o guard
/// devolvido estiver vivo.
pub fn set_xdg_config_home(path: &Path) -> EnvVarGuard {
    set_env("XDG_CONFIG_HOME", path)
}

/// Aponta `dirs::data_dir()` pra um diretório temporário enquanto o guard
/// devolvido estiver vivo.
pub fn set_xdg_data_home(path: &Path) -> EnvVarGuard {
    set_env("XDG_DATA_HOME", path)
}
