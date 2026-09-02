use ksni::TrayMethods;

use crate::daemon::DaemonEvent;

pub struct PrintcherTray {
    tx: async_channel::Sender<DaemonEvent>,
    configure_tx: async_channel::Sender<()>,
}

impl ksni::Tray for PrintcherTray {
    // Clique esquerdo já abre o menu, sem uma ação "principal" separada.
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "com.printcher.Printcher".into()
    }

    fn title(&self) -> String {
        "printcher".into()
    }

    fn icon_name(&self) -> String {
        "camera-photo-symbolic".into()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Capturar agora".into(),
                icon_name: "camera-photo-symbolic".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send_blocking(DaemonEvent::Capture);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Configurar atalho".into(),
                icon_name: "preferences-desktop-keyboard-symbolic".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.configure_tx.send_blocking(());
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Configurações".into(),
                icon_name: "preferences-system-symbolic".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send_blocking(DaemonEvent::OpenSettings);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Sair".into(),
                icon_name: "application-exit-symbolic".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send_blocking(DaemonEvent::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Sobe o ícone da bandeja (StatusNotifierItem). O `Handle` retornado precisa
/// ser mantido vivo pelo resto da vida do daemon — soltá-lo remove o ícone.
pub async fn spawn(
    tx: async_channel::Sender<DaemonEvent>,
    configure_tx: async_channel::Sender<()>,
) -> anyhow::Result<ksni::Handle<PrintcherTray>> {
    let tray = PrintcherTray { tx, configure_tx };
    Ok(tray.spawn().await?)
}
