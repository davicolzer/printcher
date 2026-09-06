//! Janela do editor de captura: construção de widgets GTK e ligação dos
//! eventos (mouse, teclado, botões) com a lógica pura em [`render`]. Não
//! tem testes automatizados aqui — depende de uma sessão gráfica real pra
//! rodar, então é validado manualmente (veja `docs/DEVELOPMENT.md`).

mod icons;
mod render;

use std::cell::RefCell;
use std::fs::File;
use std::rc::Rc;

use gtk::cairo;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

use render::{AppState, Annotation, CropRect, Point, Tool};

/// Abre uma janela do editor de captura (crop + anotações) sobre a imagem
/// congelada, associada à `Application` do daemon já em execução. Não
/// bloqueia: a janela fica sob o controle do loop principal do GTK que já
/// está rodando. `runtime` é usado só pra mandar notificações do sistema
/// (Salvar/Copiar rodam no thread principal do GTK, não numa task tokio).
/// `window_slot` guarda a janela do editor atualmente aberta (se houver):
/// uma nova captura antes de salvar/cancelar a anterior fecha a antiga
/// primeiro -- ter duas janelas de editor ao mesmo tempo é o que deixava a
/// mais nova sem foco nenhum (clique e Esc iam pra janela escondida atrás).
pub fn open_editor_window(
    app: &gtk::Application,
    image_path: std::path::PathBuf,
    runtime: tokio::runtime::Handle,
    window_slot: &Rc<RefCell<Option<gtk::ApplicationWindow>>>,
) -> anyhow::Result<()> {
    // Mesmo cuidado que em daemon.rs::handle_capture: não dá pra fazer isso
    // num `if let` só, porque o `RefMut` fica vivo até o fim do bloco e
    // `old.destroy()` dispara "destroy" na hora, que tenta pegar esse mesmo
    // RefCell emprestado de novo (ver `connect_destroy` no fim desta
    // função) -- panic de "RefCell already borrowed".
    let old = window_slot.borrow_mut().take();
    if let Some(old) = old {
        old.destroy();
    }

    let mut file = File::open(&image_path)?;
    let image = cairo::ImageSurface::create_from_png(&mut file)
        .map_err(|e| anyhow::anyhow!("falha ao carregar captura: {e:?}"))?;
    let temp_path = image_path.clone();

    // Ferramenta inicial é Cortar (não Selecionar): reproduz o fluxo do
    // GNOME Screenshot/Lightshot -- a tela já congela pronta pra você
    // arrastar uma seleção, sem precisar clicar em nada primeiro.
    let state = Rc::new(RefCell::new(AppState {
        image,
        tool: Tool::Crop,
        color: (0.9, 0.1, 0.1),
        stroke_width: 4.0,
        annotations: Vec::new(),
        crop: None,
        drag_start: None,
        drag_current: None,
    }));

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("printcher — editor de captura")
        .build();

    // A imagem ocupa a janela inteira; a barra de ferramentas flutua por
    // cima (como um dock), em vez de dividir o espaço com ela -- por isso
    // `Overlay` no lugar de uma Box vertical simples.
    let overlay = gtk::Overlay::new();
    window.set_child(Some(&overlay));

    // Sem ScrolledWindow: a imagem (em resolução física, que em telas HiDPI
    // é maior do que a área lógica disponível) é sempre escalada pra caber
    // inteira na janela -- ver `scale_factor()`/o draw_func abaixo. Isso
    // evita o corte que acontecia antes (a área de desenho pedia o tamanho
    // físico da imagem, maior que o espaço lógico da tela, e ficava sem
    // rolagem visível pro resto).
    let area = gtk::DrawingArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);
    overlay.set_child(Some(&area));

    // Dock flutuante no topo, centralizado -- estilo "osd" (usado em
    // players/visualizadores de imagem do GNOME pra controles sobre o
    // conteúdo): fundo translúcido arredondado que não precisa de nenhum
    // espaço próprio no layout, então a imagem sempre usa a janela inteira.
    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    toolbar.set_margin_top(8);
    toolbar.set_margin_bottom(8);
    toolbar.set_margin_start(10);
    toolbar.set_margin_end(10);
    toolbar.add_css_class("osd");
    toolbar.add_css_class("toolbar");
    toolbar.set_halign(gtk::Align::Center);
    toolbar.set_valign(gtk::Align::Start);
    toolbar.set_margin_top(18);
    overlay.add_overlay(&toolbar);

    // Calcula o fator de escala pra caber a imagem inteira na área
    // disponível (nunca corta, encolhe ou aumenta conforme o necessário).
    let scale_factor = {
        let state = state.clone();
        let area = area.clone();
        move || -> f64 {
            let (img_w, img_h) = {
                let state = state.borrow();
                (state.image.width() as f64, state.image.height() as f64)
            };
            let (aw, ah) = (area.width() as f64, area.height() as f64);
            if img_w <= 0.0 || img_h <= 0.0 || aw <= 0.0 || ah <= 0.0 {
                1.0
            } else {
                (aw / img_w).min(ah / img_h)
            }
        }
    };

    // --- Ferramentas (botões agrupados) ---
    let tools: [(&str, Tool, gdk::Texture); 7] = [
        ("Selecionar", Tool::Select, icons::select()),
        ("Cortar", Tool::Crop, icons::crop()),
        ("Linha", Tool::Line, icons::line()),
        ("Seta", Tool::Arrow, icons::arrow()),
        ("Retângulo", Tool::Rect, icons::rect()),
        ("Elipse", Tool::Ellipse, icons::ellipse()),
        ("Texto", Tool::Text, icons::text()),
    ];
    let mut leader: Option<gtk::ToggleButton> = None;
    let mut crop_btn: Option<gtk::ToggleButton> = None;
    for (label, tool, texture) in tools {
        let btn = gtk::ToggleButton::builder()
            .child(&gtk::Image::from_paintable(Some(&texture)))
            .tooltip_text(label)
            .build();
        if let Some(ref l) = leader {
            btn.set_group(Some(l));
        } else {
            leader = Some(btn.clone());
        }
        if tool == Tool::Crop {
            crop_btn = Some(btn.clone());
        }
        let state = state.clone();
        let area_clone = area.clone();
        btn.connect_toggled(move |b| {
            if b.is_active() {
                state.borrow_mut().tool = tool;
                area_clone.queue_draw();
            }
        });
        toolbar.append(&btn);
    }
    // Ativa Cortar por padrão (em vez do primeiro botão criado) -- reflete
    // o `tool: Tool::Crop` já definido no AppState inicial acima.
    if let Some(btn) = crop_btn {
        btn.set_active(true);
    }

    // --- Seletor de cor ---
    let color_dialog = gtk::ColorDialog::new();
    let color_btn = gtk::ColorDialogButton::new(Some(color_dialog));
    color_btn.set_rgba(&gdk::RGBA::new(0.9, 0.1, 0.1, 1.0));
    {
        let state = state.clone();
        color_btn.connect_rgba_notify(move |b| {
            let rgba = b.rgba();
            state.borrow_mut().color = (rgba.red() as f64, rgba.green() as f64, rgba.blue() as f64);
        });
    }
    toolbar.append(&color_btn);

    // --- Desfazer ---
    let undo_btn = gtk::Button::builder()
        .child(&gtk::Image::from_paintable(Some(&icons::undo())))
        .tooltip_text("Desfazer")
        .build();
    {
        let state = state.clone();
        let area_clone = area.clone();
        undo_btn.connect_clicked(move |_| {
            render::undo(&mut state.borrow_mut());
            area_clone.queue_draw();
        });
    }
    toolbar.append(&undo_btn);

    let cancel_btn = gtk::Button::builder()
        .child(&gtk::Image::from_paintable(Some(&icons::cancel())))
        .tooltip_text("Cancelar")
        .build();
    {
        let window = window.clone();
        cancel_btn.connect_clicked(move |_| window.close());
    }
    toolbar.append(&cancel_btn);

    let copy_btn = gtk::Button::builder()
        .child(&gtk::Image::from_paintable(Some(&icons::copy())))
        .tooltip_text("Copiar")
        .build();
    {
        let state = state.clone();
        let runtime = runtime.clone();
        copy_btn.connect_clicked(move |btn| {
            let result = render::compose_final(&state.borrow()).and_then(|surface| {
                let bytes = render::encode_png_bytes(&surface)?;
                let texture = gdk::Texture::from_bytes(&glib::Bytes::from(&bytes))
                    .map_err(|e| anyhow::anyhow!("falha ao criar textura: {e}"))?;
                btn.clipboard().set_texture(&texture);
                Ok(())
            });
            match result {
                Ok(()) => notify(&runtime, "editor-copy", "Copiado", "A captura foi copiada para a área de transferência.".to_string()),
                Err(e) => {
                    eprintln!("Erro ao copiar: {e}");
                    notify(&runtime, "editor-copy", "Falha ao copiar", e.to_string());
                }
            }
        });
    }
    toolbar.append(&copy_btn);

    let save_btn = gtk::Button::builder()
        .child(&gtk::Image::from_paintable(Some(&icons::save())))
        .tooltip_text("Salvar")
        .build();
    {
        let state = state.clone();
        let window = window.clone();
        let runtime = runtime.clone();
        save_btn.connect_clicked(move |_| {
            // Salva num arquivo novo em ~/Pictures/printcher/ (não sobrescreve
            // o arquivo temporário da captura crua) -- só o resultado final
            // (já cortado/anotado) deve acabar lá.
            let result = crate::capture::dest_path()
                .and_then(|dest| render::compose_final(&state.borrow()).and_then(|surface| render::save_surface(&surface, &dest)));
            match result {
                Ok(()) => {
                    notify(&runtime, "editor-save", "Captura salva", "Salva com sucesso.".to_string());
                    window.close();
                }
                Err(e) => {
                    eprintln!("Erro ao salvar: {e}");
                    notify(&runtime, "editor-save", "Falha ao salvar", e.to_string());
                }
            }
        });
    }
    toolbar.append(&save_btn);

    // --- Desenho ---
    {
        let state = state.clone();
        let scale_factor = scale_factor.clone();
        area.set_draw_func(move |_area, cr, _w, _h| {
            let state = state.borrow();
            let scale = scale_factor();

            let _ = cr.save();
            cr.scale(scale, scale);

            let _ = cr.set_source_surface(&state.image, 0.0, 0.0);
            let _ = cr.paint();

            for ann in &state.annotations {
                let _ = render::draw_annotation(cr, ann);
            }

            if let (Some(start), Some(current)) = (state.drag_start, state.drag_current) {
                if matches!(state.tool, Tool::Line | Tool::Arrow | Tool::Rect | Tool::Ellipse) {
                    let preview = render::make_annotation(state.tool, start, current, state.color, state.stroke_width);
                    if let Some(ann) = preview {
                        let _ = render::draw_annotation(cr, &ann);
                    }
                }
            }

            // Enquanto uma nova seleção de corte está sendo arrastada, ela
            // tem prioridade sobre o corte já confirmado -- sem isso, refazer
            // o corte (depois de já ter um) não mostrava o pontilhado se
            // mexendo durante o arrasto, só depois de soltar o botão (o
            // corte antigo, já salvo em `state.crop`, ficava sempre por
            // cima).
            let (img_w, img_h) = (state.image.width() as f64, state.image.height() as f64);
            if state.tool == Tool::Crop {
                if let (Some(start), Some(current)) = (state.drag_start, state.drag_current) {
                    render::draw_crop_overlay(cr, &CropRect { p0: start, p1: current }, img_w, img_h);
                } else if let Some(r) = &state.crop {
                    render::draw_crop_overlay(cr, r, img_w, img_h);
                }
            } else if let Some(r) = &state.crop {
                render::draw_crop_overlay(cr, r, img_w, img_h);
            }

            let _ = cr.restore();
        });
    }

    // --- Gesto de arrastar (linha, seta, retângulo, elipse, corte) ---
    // Coordenadas do GTK chegam em espaço do widget (pixels lógicos da
    // tela); dividimos pelo fator de escala pra guardar tudo em espaço da
    // imagem (resolução física), que é o que `render::` e o arquivo final
    // esperam -- ver comentário em `scale_factor` acima.
    let drag = gtk::GestureDrag::new();
    {
        let state = state.clone();
        let scale_factor = scale_factor.clone();
        drag.connect_drag_begin(move |_, x, y| {
            let scale = scale_factor();
            let mut state = state.borrow_mut();
            if matches!(state.tool, Tool::Select | Tool::Text) {
                return;
            }
            let pos = (x / scale, y / scale);
            state.drag_start = Some(pos);
            state.drag_current = Some(pos);
        });
    }
    {
        let state = state.clone();
        let area_clone = area.clone();
        let scale_factor = scale_factor.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            let scale = scale_factor();
            let mut state = state.borrow_mut();
            if let Some(start) = state.drag_start {
                state.drag_current = Some((start.0 + dx / scale, start.1 + dy / scale));
                drop(state);
                area_clone.queue_draw();
            }
        });
    }
    {
        let state = state.clone();
        let area_clone = area.clone();
        let scale_factor = scale_factor.clone();
        drag.connect_drag_end(move |_, dx, dy| {
            let scale = scale_factor();
            let mut state = state.borrow_mut();
            if let Some(start) = state.drag_start {
                let end = (start.0 + dx / scale, start.1 + dy / scale);
                match state.tool {
                    Tool::Crop => state.crop = Some(CropRect { p0: start, p1: end }),
                    Tool::Line | Tool::Arrow | Tool::Rect | Tool::Ellipse => {
                        if let Some(ann) = render::make_annotation(state.tool, start, end, state.color, state.stroke_width) {
                            state.annotations.push(ann);
                        }
                    }
                    Tool::Select | Tool::Text => {}
                }
            }
            state.drag_start = None;
            state.drag_current = None;
            drop(state);
            area_clone.queue_draw();
        });
    }
    area.add_controller(drag);

    // --- Clique (texto) ---
    let click = gtk::GestureClick::new();
    {
        let state = state.clone();
        let area_clone = area.clone();
        let scale_factor = scale_factor.clone();
        click.connect_released(move |_, n_press, x, y| {
            if n_press != 1 {
                return;
            }
            if state.borrow().tool != Tool::Text {
                return;
            }
            let scale = scale_factor();
            open_text_popover(&area_clone, &state, (x / scale, y / scale), (x, y));
        });
    }
    area.add_controller(click);

    // --- Atalhos de teclado ---
    let key_controller = gtk::EventControllerKey::new();
    {
        let state = state.clone();
        let area_clone = area.clone();
        let window = window.clone();
        let save_btn = save_btn.clone();
        let copy_btn = copy_btn.clone();
        key_controller.connect_key_pressed(move |_, key, _, modifiers| {
            if key == gdk::Key::Escape {
                window.close();
                return glib::Propagation::Stop;
            }
            if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
                if key == gdk::Key::z {
                    render::undo(&mut state.borrow_mut());
                    area_clone.queue_draw();
                    return glib::Propagation::Stop;
                }
                if key == gdk::Key::s {
                    save_btn.emit_clicked();
                    return glib::Propagation::Stop;
                }
                if key == gdk::Key::c {
                    copy_btn.emit_clicked();
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
    }
    window.add_controller(key_controller);

    // Independente de como a janela fecha (Cancelar, Salvar, Esc, ou o X da
    // janela), o arquivo temporário da captura crua não serve mais pra nada
    // -- ver `capture::temp_capture_path`. Ignora erro se já tiver sido
    // removido (ex: chamado duas vezes).
    window.connect_close_request(move |_| {
        let _ = std::fs::remove_file(&temp_path);
        glib::Propagation::Proceed
    });

    // Registra esta janela como "a" janela do editor atual, e limpa o slot
    // quando ela for destruída -- é o que permite `open_editor_window`
    // fechar automaticamente uma janela de edição anterior ainda aberta.
    {
        let window_slot_for_destroy = window_slot.clone();
        window.connect_destroy(move |_| {
            window_slot_for_destroy.borrow_mut().take();
        });
        *window_slot.borrow_mut() = Some(window.clone());
    }

    window.fullscreen();
    window.present();
    Ok(())
}

/// Manda uma notificação do sistema em segundo plano (dispara e esquece),
/// sem bloquear o thread do GTK.
fn notify(runtime: &tokio::runtime::Handle, id: &'static str, title: &'static str, body: String) {
    runtime.spawn(async move {
        let _ = crate::notify::send(id, title, &body).await;
    });
}

/// `image_pos` (espaço da imagem, resolução física) é o que fica guardado
/// na anotação; `widget_pos` (espaço do widget, pixels lógicos da tela) é
/// só pra posicionar o popover no lugar certo da tela -- podem divergir em
/// telas HiDPI, já que a imagem é desenhada escalada (ver `scale_factor`).
fn open_text_popover(area: &gtk::DrawingArea, state: &Rc<RefCell<AppState>>, image_pos: Point, widget_pos: Point) {
    let entry = gtk::Entry::new();
    entry.set_width_chars(24);

    let popover = gtk::Popover::new();
    popover.set_parent(area);
    popover.set_pointing_to(Some(&gdk::Rectangle::new(widget_pos.0 as i32, widget_pos.1 as i32, 1, 1)));
    popover.set_child(Some(&entry));

    {
        let state = state.clone();
        let area = area.clone();
        let popover_weak = popover.downgrade();
        entry.connect_activate(move |entry| {
            let text = entry.text().to_string();
            if !text.is_empty() {
                let mut state = state.borrow_mut();
                let color = state.color;
                state.annotations.push(Annotation::Text {
                    pos: image_pos,
                    text,
                    color,
                    size: 28.0,
                });
            }
            if let Some(popover) = popover_weak.upgrade() {
                popover.popdown();
            }
            area.queue_draw();
        });
    }

    popover.popup();
    entry.grab_focus();
}
