use adw::prelude::*;
use gtk::glib;

use crate::daemon::DaemonEvent;
use crate::{autostart, config};

// `PreferencesWindow` está depreciada desde libadwaita 1.6 em favor de
// `PreferencesDialog`, mas essa exige uma janela-pai pra `present()` — o
// daemon nem sempre tem uma aberta (pode ser a primeira janela a aparecer),
// e não deu pra validar visualmente ainda se `present(None)` funciona bem.
// Mantendo `PreferencesWindow` (ainda funcional) até validar isso na tela.
#[allow(deprecated)]
/// Abre a janela de configurações do printcher, associada à `Application` do
/// daemon já em execução. Não bloqueia.
///
/// Pensada pra crescer: cada grupo de configuração é um `PreferencesGroup`
/// independente — novas funções (upload, pasta de destino, cor padrão de
/// anotação, etc.) entram como um novo grupo/linha, sem mexer no resto.
pub fn open_settings_window(
    app: &gtk::Application,
    tx: async_channel::Sender<DaemonEvent>,
    configure_shortcut_tx: async_channel::Sender<()>,
    is_first_run: bool,
) {
    let cfg = config::load();

    // Altura generosa o suficiente pra caber os 3 grupos (boas-vindas,
    // atalho, geral) sem precisar rolar na primeira execução — com 320px
    // (valor anterior) o grupo "Geral" ficava fora da área visível.
    let window = adw::PreferencesWindow::builder()
        .application(app)
        .title("Configurações — printcher")
        .default_width(480)
        .default_height(480)
        .build();

    let page = adw::PreferencesPage::new();
    window.add(&page);

    if is_first_run {
        page.add(&welcome_group());
    }
    page.add(&shortcut_group(configure_shortcut_tx));
    page.add(&general_group(cfg.start_on_login));

    // O printcher é feito pra ficar em segundo plano (atalho global, tray),
    // então fechar a janela não deve encerrar o processo sem perguntar.
    window.connect_close_request(move |window| {
        let window = window.clone();
        let tx = tx.clone();
        glib::spawn_future_local(async move {
            confirm_close(&window, &tx).await;
        });
        glib::Propagation::Stop
    });

    window.present();
}

/// Pergunta se o usuário quer encerrar o printcher por completo ou só
/// fechar a janela (deixando o daemon rodando em segundo plano).
#[allow(deprecated)]
async fn confirm_close(window: &adw::PreferencesWindow, tx: &async_channel::Sender<DaemonEvent>) {
    const CANCEL: i32 = 0;
    const BACKGROUND: i32 = 1;
    const QUIT: i32 = 2;

    let dialog = gtk::AlertDialog::builder()
        .message("Fechar a janela de configurações")
        .detail("O printcher pode continuar rodando em segundo plano pra manter o atalho global e o ícone na bandeja ativos.")
        .buttons(["Cancelar", "Deixar em segundo plano", "Encerrar completamente"])
        .cancel_button(CANCEL)
        .default_button(BACKGROUND)
        .modal(true)
        .build();

    match dialog.choose_future(Some(window)).await {
        Ok(BACKGROUND) => window.destroy(),
        Ok(QUIT) => {
            window.destroy();
            let _ = tx.send(DaemonEvent::Quit).await;
        }
        _ => {}
    }
}

/// Grupo mostrado só na primeira execução, chamando atenção pro grupo de
/// atalho logo abaixo — é o único passo manual que o usuário precisa fazer
/// (o resto, tipo autostart, já fica ligado sozinho).
fn welcome_group() -> adw::PreferencesGroup {
    adw::PreferencesGroup::builder()
        .title("Bem-vindo ao printcher!")
        .description("Configure seu atalho de captura logo abaixo pra começar a usar.")
        .build()
}

/// Grupo "Atalho de captura": a tecla em si é configurada pela UI nativa do
/// sistema (portal GlobalShortcuts) — aqui só temos o botão que abre ela.
fn shortcut_group(configure_shortcut_tx: async_channel::Sender<()>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Atalho de captura")
        .description("A tecla é definida pela configuração de atalhos do seu sistema")
        .build();

    let row = adw::ActionRow::builder()
        .title("Tecla de captura")
        .subtitle("Abre a tela de atalhos do GNOME/KDE")
        .activatable(true)
        .build();

    let configure_btn = gtk::Button::builder()
        .label("Configurar…")
        .valign(gtk::Align::Center)
        .build();
    configure_btn.connect_clicked(move |_| {
        let _ = configure_shortcut_tx.send_blocking(());
    });

    row.add_suffix(&configure_btn);
    row.set_activatable_widget(Some(&configure_btn));
    group.add(&row);

    group
}

/// Grupo "Geral": por enquanto só "iniciar com o sistema", mas é onde
/// futuras opções gerais (não ligadas a uma função específica) entram.
fn general_group(start_on_login: bool) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("Geral").build();

    let row = adw::SwitchRow::builder()
        .title("Iniciar com o sistema")
        .subtitle("Sobe o printcher em segundo plano no login")
        .active(start_on_login)
        .build();

    row.connect_active_notify(|row| {
        let active = row.is_active();

        let mut cfg = config::load();
        cfg.start_on_login = active;
        if let Err(e) = config::save(&cfg) {
            eprintln!("Erro ao salvar configurações: {e}");
        }

        let result = if active { autostart::install() } else { autostart::uninstall() };
        if let Err(e) = result {
            eprintln!("Erro ao atualizar autostart: {e}");
        }
    });

    group.add(&row);
    group
}
