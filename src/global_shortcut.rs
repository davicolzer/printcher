use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;

use crate::daemon::DaemonEvent;

const SHORTCUT_ID: &str = "capture";
const SHORTCUT_DESCRIPTION: &str = "Capturar tela";
// Sugestão de tecla padrão (Print Screen) pro compositor pré-preencher na
// primeira configuração -- é só uma dica, não uma imposição: o compositor
// decide se usa, e ainda mostra a tela de confirmação pro usuário (inclusive
// avisando se PrtScr já está em uso pela ferramenta de captura nativa do
// sistema, com opção de substituir). Não temos como pular essa confirmação
// nem desligar o atalho nativo por código -- é assim que o portal funciona
// no Wayland, de propósito.
const PREFERRED_TRIGGER: &str = "Print";

/// Registra o atalho global de captura via `org.freedesktop.portal.GlobalShortcuts`
/// e fica escutando ativações, encaminhando-as pro canal de eventos do
/// daemon. Também escuta pedidos de reconfiguração (`configure_rx`), que
/// abrem a UI nativa do sistema pra remapear a tecla. Roda indefinidamente —
/// deve ser colocado numa task própria do runtime tokio.
pub async fn run(
    tx: async_channel::Sender<DaemonEvent>,
    configure_rx: async_channel::Receiver<()>,
) -> anyhow::Result<()> {
    let portal = GlobalShortcuts::new().await?;
    let session = portal.create_session(Default::default()).await?;

    let shortcuts = [NewShortcut::new(SHORTCUT_ID, SHORTCUT_DESCRIPTION).preferred_trigger(PREFERRED_TRIGGER)];
    portal
        .bind_shortcuts(&session, &shortcuts, None, Default::default())
        .await?
        .response()?;

    let mut activated = portal.receive_activated().await?;

    loop {
        tokio::select! {
            event = activated.next() => {
                let Some(event) = event else { break };
                if event.shortcut_id() == SHORTCUT_ID {
                    let _ = tx.send(DaemonEvent::Capture).await;
                }
            }
            request = configure_rx.recv() => {
                if request.is_err() {
                    break;
                }
                if let Err(e) = portal
                    .configure_shortcuts(&session, None, Default::default())
                    .await
                {
                    eprintln!("Erro ao abrir configuração de atalhos: {e}");
                }
            }
        }
    }

    Ok(())
}
