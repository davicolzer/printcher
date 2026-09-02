mod capture;
mod editor;
mod shortcut;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("--install-shortcut") => return shortcut::install(args.get(2).cloned()),
        Some("--uninstall-shortcut") => return shortcut::uninstall(),
        _ => {}
    }

    let image_path = capture::capture_fullscreen().await?;
    editor::run_editor(image_path)
}
