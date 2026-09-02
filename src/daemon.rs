use gtk::glib;
use gtk::prelude::*;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::interface;

use crate::{capture, editor, global_shortcut, tray};

const BUS_NAME: &str = "com.printcher.Printcher";
const OBJECT_PATH: &str = "/com/printcher/Printcher";
const INTERFACE_NAME: &str = "com.printcher.Printcher";
const METHOD_CAPTURE: &str = "Capture";
const METHOD_QUIT: &str = "Quit";
const METHOD_CONFIGURE_SHORTCUT: &str = "ConfigureShortcut";

#[derive(Debug, Clone, Copy)]
pub(crate) enum DaemonEvent {
    Capture,
    Quit,
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
}

/// Roda o printcher como daemon de instância única.
///
/// Se já existir uma instância ativa, apenas pede pra ela capturar (quando
/// `initial_capture` for true) e retorna. Caso contrário, assume o papel de
/// daemon, registra a interface D-Bus, liga o atalho global via portal,
/// opcionalmente dispara uma captura inicial, e fica residente escutando
/// eventos até ser encerrado.
pub fn run(initial_capture: bool) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let (tx, rx) = async_channel::unbounded::<DaemonEvent>();
    let (configure_tx, configure_rx) = async_channel::unbounded::<()>();

    let connection = runtime.block_on(become_primary(tx.clone(), configure_tx.clone()))?;
    let Some(connection) = connection else {
        if initial_capture {
            runtime.block_on(call_remote(METHOD_CAPTURE))?;
        }
        return Ok(());
    };

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
    let tray_handle = match runtime.block_on(tray::spawn(tx.clone(), configure_tx)) {
        Ok(handle) => Some(handle),
        Err(e) => {
            eprintln!("Ícone da bandeja indisponível: {e}");
            None
        }
    };

    if initial_capture {
        tx.send_blocking(DaemonEvent::Capture)?;
    }

    run_gtk_loop(runtime, connection, tray_handle, rx)
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
    rx: async_channel::Receiver<DaemonEvent>,
) -> anyhow::Result<()> {
    // NON_UNIQUE: a instância única já é garantida por nós mesmos (posse do
    // nome com.printcher.Printcher via zbus, acima). Sem essa flag, o
    // GApplication tenta possuir o mesmo nome pra sua própria checagem de
    // unicidade e entra em conflito com a nossa conexão.
    let app = gtk::Application::builder()
        .application_id("com.printcher.Printcher")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();

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
        glib::spawn_future_local(async move {
            while let Ok(event) = rx.recv().await {
                match event {
                    DaemonEvent::Capture => handle_capture(&app, &runtime_handle).await,
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
            if let Err(e) = editor::open_editor_window(app, path) {
                eprintln!("Erro ao abrir o editor: {e}");
            }
        }
        Ok(Err(e)) => eprintln!("Erro na captura: {e}"),
        Err(_) => {}
    }
}
