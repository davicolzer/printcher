mod autostart;
mod capture;
mod config;
mod daemon;
mod editor;
mod global_shortcut;
mod launcher;
mod notify;
mod settings_window;
#[cfg(test)]
mod testutil;
mod tray;

use daemon::InitialAction;

fn main() -> anyhow::Result<()> {
    // Em algumas máquinas o renderizador Vulkan do GTK4 falha silenciosamente
    // (janela criada e "visível", mas nunca desenha nada na tela --
    // `vkAcquireNextImageKHR` retornando VK_ERROR_OUT_OF_DATE_KHR). OpenGL é
    // mais amplamente suportado e não tem esse problema; só força se o
    // usuário não tiver uma preferência própria já setada.
    if std::env::var_os("GSK_RENDERER").is_none() {
        std::env::set_var("GSK_RENDERER", "gl");
    }

    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("--install-autostart") => autostart::install(),
        Some("--uninstall-autostart") => autostart::uninstall(),
        Some("--install-launcher") => launcher::install(),
        Some("--uninstall-launcher") => launcher::uninstall(),
        Some("--uninstall-all") => uninstall_all(),
        Some("--daemon") => daemon::run(None),
        Some("--quit") => daemon::request_quit(),
        Some("--configure-shortcut") => daemon::request_configure_shortcut(),
        Some("--settings") => daemon::run(Some(InitialAction::OpenSettings)),
        _ => daemon::run_default(),
    }
}

/// Remove autostart, ícone do launcher e configurações salvas. Não mexe nos
/// screenshots em `~/Pictures/printcher/` (conteúdo do usuário).
fn uninstall_all() -> anyhow::Result<()> {
    autostart::uninstall()?;
    launcher::uninstall()?;
    config::remove_all()?;
    println!("printcher desinstalado (autostart, launcher e configurações removidos).");
    Ok(())
}
