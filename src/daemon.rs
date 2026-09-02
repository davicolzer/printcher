use gtk::glib;
use gtk::prelude::*;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::interface;

use crate::{capture, editor, global_shortcut, settings_window, tray};

const BUS_NAME: &str = "com.printcher.Printcher";
const OBJECT_PATH: &str = "/com/printcher/Printcher";
const INTERFACE_NAME: &str = "com.printcher.Printcher";
const METHOD_CAPTURE: &str = "Capture";
const METHOD_QUIT: &str = "Quit";
const METHOD_CONFIGURE_SHORTCUT: &str = "ConfigureShortcut";
const METHOD_OPEN_SETTINGS: &str = "OpenSettings";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DaemonEvent {
    Capture,
    Quit,
    OpenSettings,
}

struct PrintcherService {
    tx: async_channel::Sender<DaemonEvent>,
    configure_tx: async_channel::Sender<()>,
}

#[interface(name = "com.printcher.Printcher")]
impl PrintcherService {
    async fn capture(&self) {
        let _ = self.tx.send(DaemonEvent::Capture).await;
    }

    async fn quit(&self) {
        let _ = self.tx.send(DaemonEvent::Quit).await;
    }

    async fn configure_shortcut(&self) {
        let _ = self.configure_tx.send(()).await;
    }

    async fn open_settings(&self) {
        let _ = self.tx.send(DaemonEvent::OpenSettings).await;
    }
}

/// O que fazer assim que o daemon estiver pronto (ou o que pedir pra uma
/// instância já existente), dependendo de como o printcher foi invocado.
#[derive(Debug, Clone, Copy)]
pub(crate) enum InitialAction {
    Capture,
    OpenSettings,
}

impl InitialAction {
    fn dbus_method(self) -> &'static str {
        match self {
            InitialAction::Capture => METHOD_CAPTURE,
            InitialAction::OpenSettings => METHOD_OPEN_SETTINGS,
        }
    }

    fn daemon_event(self) -> DaemonEvent {
        match self {
            InitialAction::Capture => DaemonEvent::Capture,
            InitialAction::OpenSettings => DaemonEvent::OpenSettings,
        }
    }
}

/// Roda o printcher como daemon de instância única.
///
/// Se já existir uma instância ativa, apenas repassa `initial_action` pra
/// ela (via D-Bus) e retorna. Caso contrário, assume o papel de daemon,
/// registra a interface D-Bus, liga o atalho global via portal, dispara
/// `initial_action` localmente, e fica residente escutando eventos até ser
/// encerrado.
pub fn run(initial_action: Option<InitialAction>) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let (tx, rx) = async_channel::unbounded::<DaemonEvent>();
    let (configure_tx, configure_rx) = async_channel::unbounded::<()>();

    let connection = runtime.block_on(become_primary(tx.clone(), configure_tx.clone()))?;
    let Some(connection) = connection else {
        if let Some(action) = initial_action {
            runtime.block_on(call_remote(action.dbus_method()))?;
        }
        return Ok(());
    };

    // Primeira vez que o printcher roda de verdade nesta máquina: liga
    // autostart por padrão (sem exigir que o usuário mexa em nada) e marca
    // pra mostrar um banner de boas-vindas na tela de configurações.
    let (_, is_first_run) = crate::config::load_or_init();

    // O atalho global (portal GlobalShortcuts) roda numa task própria do
    // runtime, em paralelo com o loop do GTK. Se o portal não estiver
    // disponível (ex: desktop sem suporte), só loga e segue sem esse atalho
    // — captura via D-Bus/tray continua funcionando normalmente.
    let shortcut_tx = tx.clone();
    runtime.handle().spawn(async move {
        if let Err(e) = global_shortcut::run(shortcut_tx, configure_rx).await {
            eprintln!("Atalho global via portal indisponível: {e}");
        }
    });

    // Ícone na bandeja (StatusNotifierItem). Sem host disponível (ex: GNOME
    // sem a extensão AppIndicator), só loga e segue sem ícone — o resto do
    // daemon funciona igual.
    let tray_handle = match runtime.block_on(tray::spawn(tx.clone(), configure_tx.clone())) {
        Ok(handle) => Some(handle),
        Err(e) => {
            eprintln!("Ícone da bandeja indisponível: {e}");
            None
        }
    };

    if let Some(action) = initial_action {
        tx.send_blocking(action.daemon_event())?;
    }

    run_gtk_loop(runtime, connection, tray_handle, tx, configure_tx, rx, is_first_run)
}

/// Pede pra uma instância em execução encerrar. Não faz nada (silenciosamente)
/// se não houver nenhuma rodando.
pub fn request_quit() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    match runtime.block_on(call_remote(METHOD_QUIT)) {
        Ok(()) => Ok(()),
        Err(e) => {
            println!("Nenhuma instância do printcher em execução ({e}).");
            Ok(())
        }
    }
}

/// Pede pra uma instância em execução abrir a UI do sistema pra reconfigurar
/// o atalho global.
pub fn request_configure_shortcut() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    match runtime.block_on(call_remote(METHOD_CONFIGURE_SHORTCUT)) {
        Ok(()) => Ok(()),
        Err(e) => {
            println!("Nenhuma instância do printcher em execução ({e}).");
            Ok(())
        }
    }
}

/// Tenta se tornar a instância primária (dona do nome no D-Bus). Retorna a
/// conexão (que precisa ser mantida viva pelo resto da vida do daemon) se
/// conseguiu, ou `None` se já existe outra instância rodando.
async fn become_primary(
    tx: async_channel::Sender<DaemonEvent>,
    configure_tx: async_channel::Sender<()>,
) -> anyhow::Result<Option<zbus::Connection>> {
    let connection = zbus::connection::Builder::session()?.build().await?;

    // Registra a interface antes de pedir o nome, pra não perder chamadas
    // que cheguem logo depois de virarmos donos do nome.
    connection
        .object_server()
        .at(OBJECT_PATH, PrintcherService { tx, configure_tx })
        .await?;

    // Com DoNotQueue, `NameTaken` é o caso normal de "já tem outra instância
    // rodando" — não é um erro de verdade, só não conseguimos o nome.
    match connection
        .request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into())
        .await
    {
        Ok(RequestNameReply::PrimaryOwner) => Ok(Some(connection)),
        Ok(_) => Ok(None),
        Err(zbus::Error::NameTaken) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

async fn call_remote(method: &str) -> anyhow::Result<()> {
    let connection = zbus::Connection::session().await?;
    connection
        .call_method(Some(BUS_NAME), OBJECT_PATH, Some(INTERFACE_NAME), method, &())
        .await?;
    Ok(())
}

fn run_gtk_loop(
    runtime: tokio::runtime::Runtime,
    connection: zbus::Connection,
    tray_handle: Option<ksni::Handle<tray::PrintcherTray>>,
    tx: async_channel::Sender<DaemonEvent>,
    configure_tx: async_channel::Sender<()>,
    rx: async_channel::Receiver<DaemonEvent>,
    is_first_run: bool,
) -> anyhow::Result<()> {
    // NON_UNIQUE: a instância única já é garantida por nós mesmos (posse do
    // nome com.printcher.Printcher via zbus, acima). Sem essa flag, o
    // GApplication tenta possuir o mesmo nome pra sua própria checagem de
    // unicidade e entra em conflito com a nossa conexão.
    let app = gtk::Application::builder()
        .application_id("com.printcher.Printcher")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();

    // libadwaita precisa ser inicializado uma vez antes de usar seus widgets
    // (adw::PreferencesWindow etc, usados na janela de configurações).
    if let Err(e) = adw::init() {
        eprintln!("Erro ao inicializar libadwaita: {e}");
    }

    // Sem janela nenhuma aberta, o GApplication encerraria o loop principal
    // logo após o "activate". Esse guard mantém o processo vivo até
    // soltarmos explicitamente (aqui, isso só acontece quando o guard é
    // dropado no fim da função, após app.run_with_args() retornar).
    let _hold = app.hold();

    let runtime_handle = runtime.handle().clone();

    app.connect_activate(move |app| {
        let app = app.clone();
        let rx = rx.clone();
        let runtime_handle = runtime_handle.clone();
        let tx = tx.clone();
        let configure_tx = configure_tx.clone();
        glib::spawn_future_local(async move {
            while let Ok(event) = rx.recv().await {
                match event {
                    DaemonEvent::Capture => handle_capture(&app, &runtime_handle).await,
                    DaemonEvent::OpenSettings => settings_window::open_settings_window(
                        &app,
                        tx.clone(),
                        configure_tx.clone(),
                        is_first_run,
                    ),
                    DaemonEvent::Quit => {
                        app.quit();
                        break;
                    }
                }
            }
        });
    });

    app.run_with_args::<&str>(&[]);

    // Mantém runtime, connection e tray_handle vivos até aqui; soltar antes
    // fecharia o serviço D-Bus (e o ícone da bandeja) no meio do caminho.
    drop(tray_handle);
    drop(connection);
    drop(runtime);
    Ok(())
}

async fn handle_capture(app: &gtk::Application, handle: &tokio::runtime::Handle) {
    let (result_tx, result_rx) = async_channel::bounded(1);
    handle.spawn(async move {
        let result = capture::capture_fullscreen().await;
        let _ = result_tx.send(result).await;
    });

    match result_rx.recv().await {
        Ok(Ok(path)) => {
            if let Err(e) = editor::open_editor_window(app, path, handle.clone()) {
                eprintln!("Erro ao abrir o editor: {e}");
                notify_error(handle, "capture-error", "Não foi possível abrir o editor", &e);
            }
        }
        Ok(Err(e)) => {
            eprintln!("Erro na captura: {e}");
            notify_error(handle, "capture-error", "Não foi possível capturar a tela", &e);
        }
        Err(_) => {}
    }
}

/// Loga e manda uma notificação de erro pro usuário (a captura roda em
/// segundo plano, sem terminal visível — sem isso, uma falha passaria
/// despercebida).
fn notify_error(handle: &tokio::runtime::Handle, id: &'static str, title: &'static str, error: &anyhow::Error) {
    let body = error.to_string();
    handle.spawn(async move {
        let _ = crate::notify::send(id, title, &body).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_maps_to_the_capture_dbus_method_and_event() {
        assert_eq!(InitialAction::Capture.dbus_method(), METHOD_CAPTURE);
        assert_eq!(InitialAction::Capture.daemon_event(), DaemonEvent::Capture);
    }

    #[test]
    fn open_settings_maps_to_the_open_settings_dbus_method_and_event() {
        assert_eq!(InitialAction::OpenSettings.dbus_method(), METHOD_OPEN_SETTINGS);
        assert_eq!(InitialAction::OpenSettings.daemon_event(), DaemonEvent::OpenSettings);
    }

    #[test]
    fn dbus_method_names_are_pascal_case_matching_the_zbus_macro_convention() {
        // A macro #[interface] converte nomes de método snake_case pra
        // PascalCase -- essas constantes precisam bater com isso pro
        // call_remote() do lado cliente acertar o método certo.
        assert_eq!(METHOD_CAPTURE, "Capture");
        assert_eq!(METHOD_QUIT, "Quit");
        assert_eq!(METHOD_CONFIGURE_SHORTCUT, "ConfigureShortcut");
        assert_eq!(METHOD_OPEN_SETTINGS, "OpenSettings");
    }
}
