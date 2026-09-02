//! Janela do editor de captura: construção de widgets GTK e ligação dos
//! eventos (mouse, teclado, botões) com a lógica pura em [`render`]. Não
//! tem testes automatizados aqui — depende de uma sessão gráfica real pra
//! rodar, então é validado manualmente (veja `docs/DEVELOPMENT.md`).

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
pub fn open_editor_window(
    app: &gtk::Application,
    image_path: std::path::PathBuf,
    runtime: tokio::runtime::Handle,
) -> anyhow::Result<()> {
    let mut file = File::open(&image_path)?;
    let image = cairo::ImageSurface::create_from_png(&mut file)
        .map_err(|e| anyhow::anyhow!("falha ao carregar captura: {e:?}"))?;
    let (img_w, img_h) = (image.width(), image.height());

    let state = Rc::new(RefCell::new(AppState {
        image,
        image_path,
        tool: Tool::Select,
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

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    window.set_child(Some(&root));

    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    toolbar.set_margin_top(6);
    toolbar.set_margin_bottom(6);
    toolbar.set_margin_start(6);
    toolbar.set_margin_end(6);
    root.append(&toolbar);

    let area = gtk::DrawingArea::new();
    area.set_content_width(img_w);
    area.set_content_height(img_h);
    area.set_hexpand(true);
    area.set_vexpand(true);

    let scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&area)
        .build();
    root.append(&scroller);

    // --- Ferramentas (botões agrupados) ---
    let tools: [(&str, Tool); 7] = [
        ("Selecionar", Tool::Select),
        ("Cortar", Tool::Crop),
        ("Linha", Tool::Line),
        ("Seta", Tool::Arrow),
        ("Retângulo", Tool::Rect),
        ("Elipse", Tool::Ellipse),
        ("Texto", Tool::Text),
    ];
    let mut leader: Option<gtk::ToggleButton> = None;
    for (label, tool) in tools {
        let btn = gtk::ToggleButton::builder().label(label).build();
        if let Some(ref l) = leader {
            btn.set_group(Some(l));
        } else {
            btn.set_active(true);
            leader = Some(btn.clone());
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
    let undo_btn = gtk::Button::with_label("Desfazer");
    {
        let state = state.clone();
        let area_clone = area.clone();
        undo_btn.connect_clicked(move |_| {
            render::undo(&mut state.borrow_mut());
            area_clone.queue_draw();
        });
    }
    toolbar.append(&undo_btn);

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    toolbar.append(&spacer);

    let cancel_btn = gtk::Button::with_label("Cancelar");
    {
        let window = window.clone();
        cancel_btn.connect_clicked(move |_| window.close());
    }
    toolbar.append(&cancel_btn);

    let copy_btn = gtk::Button::with_label("Copiar");
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

    let save_btn = gtk::Button::with_label("Salvar");
    {
        let state = state.clone();
        let window = window.clone();
        let runtime = runtime.clone();
        save_btn.connect_clicked(move |_| {
            let result = render::compose_final(&state.borrow())
                .and_then(|surface| render::save_surface(&surface, &state.borrow().image_path));
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
        area.set_draw_func(move |_area, cr, _w, _h| {
            let state = state.borrow();
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

            let (img_w, img_h) = (state.image.width() as f64, state.image.height() as f64);
            if let Some(r) = &state.crop {
                render::draw_crop_overlay(cr, r, img_w, img_h);
            } else if state.tool == Tool::Crop {
                if let (Some(start), Some(current)) = (state.drag_start, state.drag_current) {
                    render::draw_crop_overlay(cr, &CropRect { p0: start, p1: current }, img_w, img_h);
                }
            }
        });
    }

    // --- Gesto de arrastar (linha, seta, retângulo, elipse, corte) ---
    let drag = gtk::GestureDrag::new();
    {
        let state = state.clone();
        drag.connect_drag_begin(move |_, x, y| {
            let mut state = state.borrow_mut();
            if matches!(state.tool, Tool::Select | Tool::Text) {
                return;
            }
            state.drag_start = Some((x, y));
            state.drag_current = Some((x, y));
        });
    }
    {
        let state = state.clone();
        let area_clone = area.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            let mut state = state.borrow_mut();
            if let Some(start) = state.drag_start {
                state.drag_current = Some((start.0 + dx, start.1 + dy));
                drop(state);
                area_clone.queue_draw();
            }
        });
    }
    {
        let state = state.clone();
        let area_clone = area.clone();
        drag.connect_drag_end(move |_, dx, dy| {
            let mut state = state.borrow_mut();
            if let Some(start) = state.drag_start {
                let end = (start.0 + dx, start.1 + dy);
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
        click.connect_released(move |_, n_press, x, y| {
            if n_press != 1 {
                return;
            }
            if state.borrow().tool != Tool::Text {
                return;
            }
            open_text_popover(&area_clone, &state, (x, y));
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

fn open_text_popover(area: &gtk::DrawingArea, state: &Rc<RefCell<AppState>>, pos: Point) {
    let entry = gtk::Entry::new();
    entry.set_width_chars(24);

    let popover = gtk::Popover::new();
    popover.set_parent(area);
    popover.set_pointing_to(Some(&gdk::Rectangle::new(pos.0 as i32, pos.1 as i32, 1, 1)));
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
                    pos,
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
