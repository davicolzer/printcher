use ashpd::desktop::Icon;
use ashpd::desktop::notification::{Button, Notification, NotificationProxy};
use futures_util::StreamExt;

/// Manda uma notificação do sistema via `org.freedesktop.portal.Notification`
/// — mesmo portal que já usamos pra Screenshot/GlobalShortcuts, sem precisar
/// de permissão nova no Flatpak.
///
/// `id` identifica a notificação (reenviar o mesmo `id` substitui a
/// anterior em vez de empilhar); use um `id` diferente por tipo de evento
/// (ex: "capture-error", "save-result") pra não descartar notificações
/// diferentes uma da outra.
pub async fn send(id: &str, title: &str, body: &str) -> anyhow::Result<()> {
    let proxy = NotificationProxy::new().await?;
    proxy
        .add_notification(id, Notification::new(title).body(body))
        .await?;
    Ok(())
}

/// Como `send`, mas com miniatura (bytes de uma imagem já codificada, ex.
/// PNG) e um botão "Abrir pasta" que abre `folder` no gerenciador de
/// arquivos quando clicado.
///
/// **Limitação de plataforma**: quem desenha o balão da notificação (cor,
/// borda, tempo até sumir sozinha) é o próprio sistema -- o portal só deixa
/// a gente escolher título, corpo, ícone e botões, não o visual em volta.
pub async fn send_saved(id: &str, title: &str, body: &str, thumbnail: Option<Vec<u8>>, folder: std::path::PathBuf) -> anyhow::Result<()> {
    let proxy = NotificationProxy::new().await?;

    let mut notification = Notification::new(title).body(body);
    if let Some(bytes) = thumbnail {
        notification = notification.icon(Icon::Bytes(bytes));
    }
    notification = notification.button(Button::new("Abrir pasta", "open-folder"));

    proxy.add_notification(id, notification).await?;

    // Escuta só até a próxima ação vinda *dessa* notificação (o `id` é
    // único por chamada -- ver `editor.rs`) -- depois disso a tarefa
    // termina sozinha, não fica um ouvinte pendurado pra sempre.
    let notification_id = id.to_string();
    let mut actions = proxy.receive_action_invoked().await?;
    while let Some(action) = actions.next().await {
        if action.id() != notification_id {
            continue;
        }
        if action.name() == "open-folder" {
            if let Err(e) = open_folder(&folder).await {
                eprintln!("Erro ao abrir pasta pela notificação: {e}");
            }
        }
        break;
    }

    Ok(())
}

/// Abre uma pasta no gerenciador de arquivos padrão, via portal
/// `org.freedesktop.portal.OpenURI` -- de dentro do sandbox do Flatpak não
/// dá pra simplesmente rodar `xdg-open`, precisa passar pelo portal.
async fn open_folder(folder: &std::path::Path) -> anyhow::Result<()> {
    let file = std::fs::File::open(folder)?;
    let proxy = ashpd::desktop::open_uri::OpenURIProxy::new().await?;
    proxy.open_directory(None, &file, Default::default()).await?;
    Ok(())
}
