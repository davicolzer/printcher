//! Janela de Configurações: padrão nativo do GNOME/libadwaita
//! (`adw::PreferencesWindow` + `AdwPreferencesGroup`/`AdwActionRow`/
//! `AdwSwitchRow`/`AdwComboRow`/`AdwEntryRow`/`AdwExpanderRow`) em vez de
//! widgets GTK simples com CSS/tema próprios -- decorada, redimensiona e se
//! adapta ao conteúdo de fábrica, tema claro/escuro/sistema já vem de graça
//! do `AdwStyleManager`, sem precisar reimplementar nada disso na mão.
//!
//! `AdwPreferencesWindow::add`/`set_visible_page` estão marcados como
//! obsoletos desde a libadwaita 1.6 (a substituta é `AdwPreferencesDialog`,
//! que precisa de uma janela "pai" persistente pra se apresentar sobre) --
//! não se aplica bem aqui, já que o printcher é um daemon de bandeja sem
//! janela principal fixa. `AdwPreferencesWindow` continua funcionando
//! normalmente, só avisando; por isso o `#![allow(deprecated)]` abaixo (o
//! tipo aparece em várias assinaturas, não só no ponto de criação).
#![allow(deprecated)]

use std::cell::RefCell;
use std::rc::Rc;
use std::time::SystemTime;

use adw::prelude::*;
use gtk::gdk;
use gtk::glib;

use crate::config::{Config, ImageFormat, Theme};
use crate::daemon::DaemonEvent;
use crate::{autostart, config};

const HISTORY_EXTENSIONS: [&str; 3] = ["png", "jpg", "webp"];
const THUMB_SIZE: u32 = 40;

/// Abre a janela de Configurações, associada à `Application` do daemon já
/// em execução. Não bloqueia.
pub fn open_settings_window(
    app: &gtk::Application,
    tx: async_channel::Sender<DaemonEvent>,
    configure_shortcut_tx: async_channel::Sender<()>,
    is_first_run: bool,
    initial_tab: &str,
    window_slot: &Rc<RefCell<Option<adw::PreferencesWindow>>>,
) {
    if let Some(existing) = window_slot.borrow().as_ref() {
        existing.present();
        return;
    }

    let cfg = config::load();
    sync_style_manager(cfg.theme);

    let window = adw::PreferencesWindow::builder()
        .application(app)
        .title("Configurações — printcher")
        .default_width(560)
        .default_height(640)
        .build();
    apply_dyslexia_font(&window, cfg.dyslexia_font);

    let cfg_page = adw::PreferencesPage::builder().title("Configurações").icon_name("preferences-system-symbolic").build();
    if is_first_run {
        cfg_page.set_description("Configure seu atalho de captura logo abaixo pra começar a usar.");
    }
    cfg_page.add(&shortcut_group(configure_shortcut_tx));
    cfg_page.add(&destination_group(&cfg, &window));
    cfg_page.add(&general_group(&cfg));
    cfg_page.add(&footer_group(tx.clone()));
    window.add(&cfg_page);

    let hist_page = adw::PreferencesPage::builder().title("Histórico").icon_name("document-open-recent-symbolic").build();
    hist_page.add(&history_group());
    window.add(&hist_page);

    if initial_tab == "hist" {
        window.set_visible_page(&hist_page);
    }

    // O printcher é feito pra ficar em segundo plano (atalho global,
    // bandeja), então fechar a janela não deve encerrar o processo sem
    // perguntar.
    {
        let tx = tx.clone();
        window.connect_close_request(move |window| {
            let window = window.clone();
            let tx = tx.clone();
            glib::spawn_future_local(async move {
                confirm_close(&window, &tx).await;
            });
            glib::Propagation::Stop
        });
    }

    // Limpa o slot quando a janela for destruída -- seja pelo fluxo normal
    // (confirm_close chamando window.destroy()) ou por um destroy() direto
    // vindo de fora (ex: daemon.rs fechando a janela de configurações
    // automaticamente ao iniciar uma captura).
    {
        let window_slot_for_destroy = window_slot.clone();
        window.connect_destroy(move |_| {
            window_slot_for_destroy.borrow_mut().take();
        });
        *window_slot.borrow_mut() = Some(window.clone());
    }

    window.present();
}

fn apply_dyslexia_font(window: &adw::PreferencesWindow, enabled: bool) {
    if enabled {
        window.add_css_class("pc-dyslexia");
    } else {
        window.remove_css_class("pc-dyslexia");
    }
}

/// O tema claro/escuro/sistema agora é só o `AdwStyleManager` global -- sem
/// paleta própria nem `CssProvider` customizado, é o mesmo tema que o resto
/// do GNOME usa.
fn sync_style_manager(theme: Theme) {
    let scheme = match theme {
        Theme::Light => adw::ColorScheme::ForceLight,
        Theme::Dark => adw::ColorScheme::ForceDark,
        Theme::System => adw::ColorScheme::Default,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
}

/// Pergunta se o usuário quer encerrar o printcher por completo ou só
/// fechar a janela (deixando o daemon rodando em segundo plano).
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

/// Grupo "Atalho de captura": continua só com o botão "Configurar…", que
/// abre a UI nativa do sistema (portal `GlobalShortcuts`) -- não dá pra
/// mostrar a tecla atualmente vinculada aqui de um jeito confiável, então
/// preferi não arriscar mostrar um valor errado.
fn shortcut_group(configure_shortcut_tx: async_channel::Sender<()>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("Atalho de captura").build();

    let configure_btn = gtk::Button::builder().label("Configurar…").valign(gtk::Align::Center).build();
    configure_btn.add_css_class("flat");
    configure_btn.connect_clicked(move |_| {
        let _ = configure_shortcut_tx.send_blocking(());
    });

    let row = adw::ActionRow::builder().title("Tecla de captura").subtitle("Abre a tela de atalhos do GNOME/KDE").build();
    row.set_activatable_widget(Some(&configure_btn));
    row.add_suffix(&configure_btn);
    group.add(&row);
    group
}

/// Grupo "Destino": pasta, formato, padrão de nome de arquivo (com um
/// expansor "Códigos e exemplos") e o switch de copiar sempre.
fn destination_group(cfg: &Config, window: &adw::PreferencesWindow) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("Destino").build();

    // --- Pasta ---
    let folder_row = adw::ActionRow::builder().title("Pasta das capturas").subtitle(crate::capture::dest_dir(cfg).display().to_string()).build();
    let choose_btn = gtk::Button::builder().label("Escolher…").valign(gtk::Align::Center).build();
    choose_btn.add_css_class("flat");
    {
        let window = window.clone();
        let folder_row = folder_row.clone();
        choose_btn.connect_clicked(move |_| {
            let dialog = gtk::FileDialog::builder().title("Escolher pasta de destino").build();
            let folder_row = folder_row.clone();
            dialog.select_folder(Some(&window), gtk::gio::Cancellable::NONE, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        let mut cfg = config::load();
                        cfg.folder = Some(path.clone());
                        if let Err(e) = config::save(&cfg) {
                            eprintln!("Erro ao salvar configurações: {e}");
                        }
                        folder_row.set_subtitle(&path.display().to_string());
                    }
                }
            });
        });
    }
    folder_row.add_suffix(&choose_btn);
    group.add(&folder_row);

    // --- Formato ---
    let format_model = gtk::StringList::new(&["PNG", "JPG", "WebP"]);
    let selected = match cfg.format {
        ImageFormat::Png => 0,
        ImageFormat::Jpg => 1,
        ImageFormat::WebP => 2,
    };
    let format_row = adw::ComboRow::builder().title("Formato").model(&format_model).selected(selected).build();
    format_row.connect_selected_notify(|row| {
        let value = match row.selected() {
            1 => ImageFormat::Jpg,
            2 => ImageFormat::WebP,
            _ => ImageFormat::Png,
        };
        let mut cfg = config::load();
        cfg.format = value;
        if let Err(e) = config::save(&cfg) {
            eprintln!("Erro ao salvar configurações: {e}");
        }
    });
    group.add(&format_row);

    // --- Padrão de nome ---
    let pattern_row = adw::EntryRow::builder().title("Padrão de nome do arquivo").text(cfg.pattern.as_str()).build();
    pattern_row.add_css_class("monospace");
    group.add(&pattern_row);

    let preview_row = adw::ActionRow::builder().title("Fica assim").sensitive(false).build();
    group.add(&preview_row);

    let refresh_preview = {
        let preview_row = preview_row.clone();
        let pattern_row = pattern_row.clone();
        move || {
            let name = crate::capture::expand_pattern(&pattern_row.text(), chrono::Local::now(), 1);
            let cfg = config::load();
            preview_row.set_subtitle(&format!("{name}.{}", cfg.format.extension()));
        }
    };
    refresh_preview();

    {
        let refresh_preview = refresh_preview.clone();
        pattern_row.connect_changed(move |row| {
            let mut cfg = config::load();
            cfg.pattern = row.text().to_string();
            if let Err(e) = config::save(&cfg) {
                eprintln!("Erro ao salvar configurações: {e}");
            }
            refresh_preview();
        });
    }
    {
        let refresh_preview = refresh_preview.clone();
        format_row.connect_selected_notify(move |_| refresh_preview());
    }

    let codes_row = adw::ExpanderRow::builder().title("Códigos e exemplos").subtitle("%Y %m %d %H %M %S %n").build();
    let tokens = [("%Y", "ano"), ("%m", "mês"), ("%d", "dia"), ("%H", "hora"), ("%M", "minuto"), ("%S", "segundo"), ("%n", "contador")];
    for (code, label) in tokens {
        let token_row = adw::ActionRow::builder().title(code).subtitle(label).activatable(true).build();
        token_row.add_css_class("monospace");
        {
            let pattern_row = pattern_row.clone();
            token_row.connect_activated(move |_| {
                let mut text = pattern_row.text().to_string();
                text.push_str(code);
                pattern_row.set_text(&text);
                pattern_row.set_position(-1);
            });
        }
        codes_row.add_row(&token_row);
    }
    let presets = ["printcher_%Y-%m-%d_%H-%M-%S", "print_%d-%m-%Y_%Hh%M", "captura_%n"];
    for preset in presets {
        let sample = crate::capture::expand_pattern(preset, chrono::Local::now(), 1);
        let preset_row = adw::ActionRow::builder().title(preset).subtitle(sample).activatable(true).build();
        preset_row.add_css_class("monospace");
        {
            let pattern_row = pattern_row.clone();
            preset_row.connect_activated(move |_| {
                pattern_row.set_text(preset);
                pattern_row.set_position(-1);
            });
        }
        codes_row.add_row(&preset_row);
    }
    group.add(&codes_row);

    // --- Copiar sempre ---
    let auto_copy_row = adw::SwitchRow::builder().title("Copiar sempre para a área de transferência").subtitle("Além de salvar o arquivo").active(cfg.auto_copy).build();
    auto_copy_row.connect_active_notify(|row| {
        let mut cfg = config::load();
        cfg.auto_copy = row.is_active();
        if let Err(e) = config::save(&cfg) {
            eprintln!("Erro ao salvar configurações: {e}");
        }
    });
    group.add(&auto_copy_row);

    group
}

fn general_group(cfg: &Config) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("Geral").build();

    let autostart_row = adw::SwitchRow::builder().title("Iniciar com o sistema").subtitle("Sobe em segundo plano no login").active(cfg.start_on_login).build();
    autostart_row.connect_active_notify(|row| {
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
    group.add(&autostart_row);

    let tray_row = adw::SwitchRow::builder()
        .title("Ícone na bandeja")
        .subtitle("Acesso rápido a capturar e configurar (vale a partir do próximo início)")
        .active(cfg.tray_enabled)
        .build();
    tray_row.connect_active_notify(|row| {
        let mut cfg = config::load();
        cfg.tray_enabled = row.is_active();
        if let Err(e) = config::save(&cfg) {
            eprintln!("Erro ao salvar configurações: {e}");
        }
    });
    group.add(&tray_row);

    let theme_model = gtk::StringList::new(&["Claro", "Escuro", "Sistema"]);
    let theme_selected = match cfg.theme {
        Theme::Light => 0,
        Theme::Dark => 1,
        Theme::System => 2,
    };
    let theme_row = adw::ComboRow::builder().title("Tema").model(&theme_model).selected(theme_selected).build();
    theme_row.connect_selected_notify(|row| {
        let value = match row.selected() {
            0 => Theme::Light,
            1 => Theme::Dark,
            _ => Theme::System,
        };
        sync_style_manager(value);
        let mut cfg = config::load();
        cfg.theme = value;
        if let Err(e) = config::save(&cfg) {
            eprintln!("Erro ao salvar configurações: {e}");
        }
    });
    group.add(&theme_row);

    let font_row = adw::SwitchRow::builder().title("Fonte amigável para dislexia").subtitle("Usa a fonte OpenDyslexic nesta janela").active(cfg.dyslexia_font).build();
    font_row.connect_active_notify(|row| {
        let active = row.is_active();
        if let Some(window) = row.root().and_then(|r| r.downcast::<adw::PreferencesWindow>().ok()) {
            apply_dyslexia_font(&window, active);
        }
        let mut cfg = config::load();
        cfg.dyslexia_font = active;
        if let Err(e) = config::save(&cfg) {
            eprintln!("Erro ao salvar configurações: {e}");
        }
    });
    group.add(&font_row);

    group
}

/// Nota de rodapé "Capturar agora" -- dispara uma captura de dentro da
/// própria janela (já em foco). Existe principalmente pra dar ao portal de
/// Screenshot uma primeira chance de mostrar o diálogo de permissão -- o
/// GNOME só permite esse diálogo quando o app pedindo está em foco, o que
/// nunca é o caso quando a captura é disparada pelo atalho global (o
/// printcher roda em segundo plano, sem janela nenhuma). Depois que a
/// permissão é concedida uma vez por aqui, capturas via atalho global
/// passam a funcionar normalmente.
fn footer_group(tx: async_channel::Sender<DaemonEvent>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .description("Os atalhos funcionam em qualquer tela do computador, mesmo com esta janela fechada.")
        .build();

    let capture_btn = gtk::Button::builder().label("Capturar agora").halign(gtk::Align::Start).build();
    capture_btn.add_css_class("suggested-action");
    capture_btn.connect_clicked(move |_| {
        let _ = tx.send_blocking(DaemonEvent::Capture);
    });
    group.add(&capture_btn);

    group
}

/// Aba de Histórico: lista os arquivos já salvos na pasta de destino
/// configurada, mais recente primeiro, com miniatura de verdade. Decodifica
/// todas de uma vez ao abrir a aba (a pasta normalmente tem poucas dezenas
/// de arquivos) -- se isso ficar perceptivelmente lento com muitas
/// capturas acumuladas, é ajuste pra uma fase futura (cache, paginação).
fn history_group() -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();

    let dir = crate::capture::dest_dir(&config::load());
    let mut entries: Vec<(std::path::PathBuf, SystemTime)> = std::fs::read_dir(&dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| HISTORY_EXTENSIONS.iter().any(|allowed| ext.eq_ignore_ascii_case(allowed)))
        })
        .filter_map(|entry| entry.metadata().ok().and_then(|m| m.modified().ok()).map(|t| (entry.path(), t)))
        .collect();
    entries.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));

    if entries.is_empty() {
        group.set_description(Some("Nenhuma captura salva ainda."));
        return group;
    }

    for (path, modified) in entries {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let row = adw::ActionRow::builder().title(&name).subtitle(relative_time(modified)).build();
        row.add_css_class("monospace");

        if let Some(texture) = thumbnail_texture(&path) {
            let picture = gtk::Picture::for_paintable(&texture);
            picture.set_content_fit(gtk::ContentFit::Cover);
            picture.set_size_request(THUMB_SIZE as i32, (THUMB_SIZE as f32 * 0.7) as i32);
            picture.add_css_class("card");
            picture.set_overflow(gtk::Overflow::Hidden);
            row.add_prefix(&picture);
        }

        group.add(&row);
    }

    group
}

/// Decodifica o arquivo (PNG/JPG/WebP, a crate `image` já sabe ler os 3) e
/// reduz pra uma miniatura pequena -- convertida direto pra
/// `gdk::MemoryTexture` a partir dos bytes RGBA, sem precisar recodificar
/// em PNG só pra exibir.
fn thumbnail_texture(path: &std::path::Path) -> Option<gdk::Texture> {
    let img = image::open(path).ok()?.thumbnail(THUMB_SIZE, THUMB_SIZE).to_rgba8();
    let (width, height) = (img.width(), img.height());
    let stride = width as usize * 4;
    let bytes = glib::Bytes::from_owned(img.into_raw());
    Some(gdk::MemoryTexture::new(width as i32, height as i32, gdk::MemoryFormat::R8g8b8a8, &bytes, stride).upcast())
}

/// "agora", "5 min", "2 h", "ontem", "5 dias" -- não traz uma crate nova só
/// pra isso.
fn relative_time(modified: SystemTime) -> String {
    let Ok(elapsed) = SystemTime::now().duration_since(modified) else {
        return "agora".to_string();
    };
    let secs = elapsed.as_secs();
    if secs < 60 {
        "agora".to_string()
    } else if secs < 3600 {
        format!("{} min", secs / 60)
    } else if secs < 3600 * 24 {
        format!("{} h", secs / 3600)
    } else if secs < 3600 * 48 {
        "ontem".to_string()
    } else {
        format!("{} dias", secs / (3600 * 24))
    }
}
