use ashpd::desktop::notification::{Notification, NotificationProxy};

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
