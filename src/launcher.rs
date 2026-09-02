use std::fs;

/// Registra o ícone do printcher no menu de aplicativos (`~/.local/share/applications/`),
/// visível pro usuário abrir manualmente. Diferente do autostart
/// (`src/autostart.rs`): esse aparece no launcher e, ao abrir, sobe o daemon
/// (se precisar) e mostra a janela de configurações — não captura.
pub fn install() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let exe = exe.to_string_lossy();

    let dir = applications_dir()?;
    fs::create_dir_all(&dir)?;

    let contents = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=printcher\n\
         Comment=Captura de tela e anotação\n\
         Exec={exe} --settings\n\
         Icon=camera-photo-symbolic\n\
         Terminal=false\n\
         Categories=Graphics;\n"
    );

    fs::write(dir.join("com.printcher.Printcher.desktop"), contents)?;
    println!("Ícone do printcher instalado no menu de aplicativos.");
    Ok(())
}

/// Remove o ícone do menu de aplicativos, se existir.
pub fn uninstall() -> anyhow::Result<()> {
    let path = applications_dir()?.join("com.printcher.Printcher.desktop");
    if path.exists() {
        fs::remove_file(path)?;
        println!("Ícone do printcher removido do menu de aplicativos.");
    } else {
        println!("Nenhum ícone do printcher encontrado no menu de aplicativos.");
    }
    Ok(())
}

fn applications_dir() -> anyhow::Result<std::path::PathBuf> {
    let data_dir = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("diretório de dados não encontrado"))?;
    Ok(data_dir.join("applications"))
}
