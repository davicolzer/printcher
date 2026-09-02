use std::fs;

/// Registra o printcher pra iniciar (como daemon, sem capturar) junto com a
/// sessão gráfica, via arquivo `.desktop` em `~/.config/autostart/`.
pub fn install() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let exe = exe.to_string_lossy();

    let dir = autostart_dir()?;
    fs::create_dir_all(&dir)?;

    let contents = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=printcher\n\
         Comment=Captura de tela em segundo plano\n\
         Exec={exe} --daemon\n\
         X-GNOME-Autostart-enabled=true\n\
         NoDisplay=true\n"
    );

    fs::write(dir.join("printcher.desktop"), contents)?;
    println!("Autostart configurado: {exe} --daemon");
    Ok(())
}

/// Remove o registro de autostart, se existir.
pub fn uninstall() -> anyhow::Result<()> {
    let path = autostart_dir()?.join("printcher.desktop");
    if path.exists() {
        fs::remove_file(path)?;
        println!("Autostart removido.");
    } else {
        println!("Nenhum autostart do printcher encontrado.");
    }
    Ok(())
}

fn autostart_dir() -> anyhow::Result<std::path::PathBuf> {
    let config_dir = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("diretório de config não encontrado"))?;
    Ok(config_dir.join("autostart"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_writes_a_valid_hidden_desktop_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::testutil::set_xdg_config_home(tmp.path());

        install().unwrap();

        let path = tmp.path().join("autostart/printcher.desktop");
        let contents = fs::read_to_string(path).unwrap();
        assert!(contents.contains("[Desktop Entry]"));
        assert!(contents.contains("Exec="));
        assert!(contents.contains("--daemon"));
        assert!(contents.contains("NoDisplay=true"));
    }

    #[test]
    fn uninstall_removes_the_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::testutil::set_xdg_config_home(tmp.path());

        install().unwrap();
        let path = tmp.path().join("autostart/printcher.desktop");
        assert!(path.exists());

        uninstall().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn uninstall_without_prior_install_does_not_error() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::testutil::set_xdg_config_home(tmp.path());

        uninstall().unwrap();
    }

    #[test]
    fn install_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::testutil::set_xdg_config_home(tmp.path());

        install().unwrap();
        install().unwrap();

        assert!(tmp.path().join("autostart/printcher.desktop").exists());
    }
}
