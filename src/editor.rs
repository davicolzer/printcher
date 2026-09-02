use std::cell::RefCell;
use std::fs::File;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::cairo;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

type Point = (f64, f64);
type Color = (f64, f64, f64);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tool {
    Select,
    Crop,
    Line,
    Arrow,
    Rect,
    Ellipse,
    Text,
}

#[derive(Debug, Clone, Copy)]
struct CropRect {
    p0: Point,
    p1: Point,
}

impl CropRect {
    /// Retorna (x0, y0, x1, y1) com x0<=x1 e y0<=y1.
    fn normalized(&self) -> (f64, f64, f64, f64) {
        let (x0, x1) = if self.p0.0 <= self.p1.0 {
            (self.p0.0, self.p1.0)
        } else {
            (self.p1.0, self.p0.0)
        };
        let (y0, y1) = if self.p0.1 <= self.p1.1 {
            (self.p0.1, self.p1.1)
        } else {
            (self.p1.1, self.p0.1)
        };
        (x0, y0, x1, y1)
    }
}

#[derive(Debug, Clone)]
enum Annotation {
    Line { p0: Point, p1: Point, color: Color, width: f64 },
    Arrow { p0: Point, p1: Point, color: Color, width: f64 },
    Rect { p0: Point, p1: Point, color: Color, width: f64 },
    Ellipse { p0: Point, p1: Point, color: Color, width: f64 },
    Text { pos: Point, text: String, color: Color, size: f64 },
}

struct AppState {
    image: cairo::ImageSurface,
    image_path: PathBuf,
    tool: Tool,
    color: Color,
    stroke_width: f64,
    annotations: Vec<Annotation>,
    crop: Option<CropRect>,
    drag_start: Option<Point>,
    drag_current: Option<Point>,
}

/// Abre o editor de captura (crop + anotações) sobre a imagem congelada.
/// Bloqueia até a janela ser fechada.
pub fn run_editor(image_path: PathBuf) -> anyhow::Result<()> {
    let app = gtk::Application::builder()
        .application_id("com.printcher.Printcher")
        .build();

    app.connect_activate(move |app| {
        if let Err(e) = build_window(app, image_path.clone()) {
            eprintln!("Erro ao abrir o editor: {e}");
        }
    });

    // Não repassa os argumentos do processo para o GTK (não usamos CLI flags).
    app.run_with_args::<&str>(&[]);
    Ok(())
}

fn build_window(app: &gtk::Application, image_path: PathBuf) -> anyhow::Result<()> {
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
            undo(&mut state.borrow_mut());
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
        copy_btn.connect_clicked(move |btn| match compose_final(&state.borrow()) {
            Ok(surface) => match encode_png_bytes(&surface) {
                Ok(bytes) => match gdk::Texture::from_bytes(&glib::Bytes::from(&bytes)) {
                    Ok(texture) => btn.clipboard().set_texture(&texture),
                    Err(e) => eprintln!("Erro ao criar textura: {e}"),
                },
                Err(e) => eprintln!("Erro ao codificar PNG: {e}"),
            },
            Err(e) => eprintln!("Erro ao compor imagem: {e}"),
        });
    }
    toolbar.append(&copy_btn);

    let save_btn = gtk::Button::with_label("Salvar");
    {
        let state = state.clone();
        let window = window.clone();
        save_btn.connect_clicked(move |_| {
            let result = compose_final(&state.borrow())
                .and_then(|surface| save_surface(&surface, &state.borrow().image_path));
            match result {
                Ok(()) => window.close(),
                Err(e) => eprintln!("Erro ao salvar: {e}"),
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
                let _ = draw_annotation(cr, ann);
            }

            if let (Some(start), Some(current)) = (state.drag_start, state.drag_current) {
                if matches!(state.tool, Tool::Line | Tool::Arrow | Tool::Rect | Tool::Ellipse) {
                    let preview = make_annotation(state.tool, start, current, state.color, state.stroke_width);
                    if let Some(ann) = preview {
                        let _ = draw_annotation(cr, &ann);
                    }
                }
            }

            let (img_w, img_h) = (state.image.width() as f64, state.image.height() as f64);
            if let Some(r) = &state.crop {
                draw_crop_overlay(cr, r, img_w, img_h);
            } else if state.tool == Tool::Crop {
                if let (Some(start), Some(current)) = (state.drag_start, state.drag_current) {
                    draw_crop_overlay(cr, &CropRect { p0: start, p1: current }, img_w, img_h);
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
                        if let Some(ann) = make_annotation(state.tool, start, end, state.color, state.stroke_width) {
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
                    undo(&mut state.borrow_mut());
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

fn undo(state: &mut AppState) {
    if state.annotations.pop().is_none() {
        state.crop = None;
    }
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

fn make_annotation(tool: Tool, p0: Point, p1: Point, color: Color, width: f64) -> Option<Annotation> {
    match tool {
        Tool::Line => Some(Annotation::Line { p0, p1, color, width }),
        Tool::Arrow => Some(Annotation::Arrow { p0, p1, color, width }),
        Tool::Rect => Some(Annotation::Rect { p0, p1, color, width }),
        Tool::Ellipse => Some(Annotation::Ellipse { p0, p1, color, width }),
        _ => None,
    }
}

fn draw_annotation(cr: &cairo::Context, ann: &Annotation) -> Result<(), cairo::Error> {
    match ann {
        Annotation::Line { p0, p1, color, width } => {
            cr.set_source_rgb(color.0, color.1, color.2);
            cr.set_line_width(*width);
            cr.move_to(p0.0, p0.1);
            cr.line_to(p1.0, p1.1);
            cr.stroke()?;
        }
        Annotation::Arrow { p0, p1, color, width } => {
            cr.set_source_rgb(color.0, color.1, color.2);
            cr.set_line_width(*width);
            cr.move_to(p0.0, p0.1);
            cr.line_to(p1.0, p1.1);
            cr.stroke()?;

            let angle = (p1.1 - p0.1).atan2(p1.0 - p0.0);
            let head_len = (*width * 4.0).max(14.0);
            let spread = std::f64::consts::PI / 7.0;
            for sign in [-1.0, 1.0] {
                let a = angle + std::f64::consts::PI - sign * spread;
                cr.move_to(p1.0, p1.1);
                cr.line_to(p1.0 + head_len * a.cos(), p1.1 + head_len * a.sin());
            }
            cr.stroke()?;
        }
        Annotation::Rect { p0, p1, color, width } => {
            cr.set_source_rgb(color.0, color.1, color.2);
            cr.set_line_width(*width);
            cr.rectangle(p0.0.min(p1.0), p0.1.min(p1.1), (p1.0 - p0.0).abs(), (p1.1 - p0.1).abs());
            cr.stroke()?;
        }
        Annotation::Ellipse { p0, p1, color, width } => {
            cr.set_source_rgb(color.0, color.1, color.2);
            cr.set_line_width(*width);
            let cx = (p0.0 + p1.0) / 2.0;
            let cy = (p0.1 + p1.1) / 2.0;
            let rx = ((p1.0 - p0.0).abs() / 2.0).max(1.0);
            let ry = ((p1.1 - p0.1).abs() / 2.0).max(1.0);
            cr.save()?;
            cr.translate(cx, cy);
            cr.scale(rx, ry);
            cr.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
            cr.restore()?;
            cr.stroke()?;
        }
        Annotation::Text { pos, text, color, size } => {
            cr.set_source_rgb(color.0, color.1, color.2);
            cr.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size(*size);
            cr.move_to(pos.0, pos.1);
            cr.show_text(text)?;
        }
    }
    Ok(())
}

fn draw_crop_overlay(cr: &cairo::Context, rect: &CropRect, img_w: f64, img_h: f64) {
    let (x0, y0, x1, y1) = rect.normalized();
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.5);
    cr.rectangle(0.0, 0.0, img_w, y0);
    cr.rectangle(0.0, y1, img_w, img_h - y1);
    cr.rectangle(0.0, y0, x0, y1 - y0);
    cr.rectangle(x1, y0, img_w - x1, y1 - y0);
    let _ = cr.fill();

    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.set_line_width(1.5);
    cr.set_dash(&[6.0, 4.0], 0.0);
    cr.rectangle(x0, y0, x1 - x0, y1 - y0);
    let _ = cr.stroke();
    cr.set_dash(&[], 0.0);
}

/// Renderiza a imagem base + anotações + corte em uma nova superfície final.
fn compose_final(state: &AppState) -> anyhow::Result<cairo::ImageSurface> {
    let (img_w, img_h) = (state.image.width() as f64, state.image.height() as f64);
    let (cx0, cy0, cx1, cy1) = match &state.crop {
        Some(r) => r.normalized(),
        None => (0.0, 0.0, img_w, img_h),
    };
    let cx0 = cx0.clamp(0.0, img_w);
    let cy0 = cy0.clamp(0.0, img_h);
    let cx1 = cx1.clamp(0.0, img_w);
    let cy1 = cy1.clamp(0.0, img_h);
    let out_w = (cx1 - cx0).max(1.0);
    let out_h = (cy1 - cy0).max(1.0);

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, out_w as i32, out_h as i32)
        .map_err(|e| anyhow::anyhow!("falha ao criar superfície: {e:?}"))?;
    let cr = cairo::Context::new(&surface).map_err(|e| anyhow::anyhow!("falha ao criar contexto: {e:?}"))?;
    cr.translate(-cx0, -cy0);
    cr.set_source_surface(&state.image, 0.0, 0.0)
        .map_err(|e| anyhow::anyhow!("falha ao desenhar base: {e:?}"))?;
    cr.paint().map_err(|e| anyhow::anyhow!("falha ao pintar base: {e:?}"))?;
    for ann in &state.annotations {
        draw_annotation(&cr, ann).map_err(|e| anyhow::anyhow!("falha ao desenhar anotação: {e:?}"))?;
    }
    drop(cr);
    Ok(surface)
}

fn save_surface(surface: &cairo::ImageSurface, path: &PathBuf) -> anyhow::Result<()> {
    let mut file = File::create(path)?;
    surface
        .write_to_png(&mut file)
        .map_err(|e| anyhow::anyhow!("falha ao gravar PNG: {e:?}"))?;
    Ok(())
}

fn encode_png_bytes(surface: &cairo::ImageSurface) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    surface
        .write_to_png(&mut buf)
        .map_err(|e| anyhow::anyhow!("falha ao codificar PNG: {e:?}"))?;
    Ok(buf)
}
