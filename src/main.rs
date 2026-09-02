mod autostart;
mod capture;
mod config;
mod daemon;
mod editor;
mod global_shortcut;
mod launcher;
mod settings_window;
mod tray;

use daemon::InitialAction;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("--install-autostart") => autostart::install(),
        Some("--uninstall-autostart") => autostart::uninstall(),
        Some("--install-launcher") => launcher::install(),
        Some("--uninstall-launcher") => launcher::uninstall(),
        Some("--daemon") => daemon::run(None),
        Some("--quit") => daemon::request_quit(),
        Some("--configure-shortcut") => daemon::request_configure_shortcut(),
        Some("--settings") => daemon::run(Some(InitialAction::OpenSettings)),
        _ => daemon::run(Some(InitialAction::Capture)),
    }
}
