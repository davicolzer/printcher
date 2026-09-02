mod capture;
mod editor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let image_path = capture::capture_fullscreen().await?;
    editor::run_editor(image_path)
}
