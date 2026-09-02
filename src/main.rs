mod autostart;
mod capture;
mod daemon;
mod editor;
mod global_shortcut;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("--install-autostart") => autostart::install(),
        Some("--uninstall-autostart") => autostart::uninstall(),
        Some("--daemon") => daemon::run(false),
        Some("--quit") => daemon::request_quit(),
        Some("--configure-shortcut") => daemon::request_configure_shortcut(),
        _ => daemon::run(true),
    }
}
