//! Ícones do editor, desenhados via Cairo em vez de carregados de um tema de
//! ícones do sistema -- garante aparência consistente e independe de quais
//! ícones simbólicos estão instalados (dev machine vs runtime do Flatpak).

use gtk::cairo;
use gtk::gdk;
use gtk::glib;

const SIZE: i32 = 20;
const STROKE: (f64, f64, f64) = (0.24, 0.24, 0.27);

fn render(draw: impl Fn(&cairo::Context)) -> gdk::Texture {
    let surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, SIZE, SIZE).expect("falha ao criar superfície do ícone");
    let cr = cairo::Context::new(&surface).expect("falha ao criar contexto do ícone");
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);

    // `draw` só monta geometria e chama fill/stroke com a cor/largura atual
    // do contexto -- por isso dá pra chamar duas vezes com ajustes
    // diferentes antes de cada uma, sem precisar reconstruir o path. Um halo
    // claro e largo desenhado antes garante contraste do traço escuro final
    // contra qualquer fundo (botão marcado com cor de destaque, tema
    // escuro, etc.) -- sem ele, o traço escuro sozinho quase sumia nesses
    // casos.
    cr.set_line_width(3.6);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    draw(&cr);

    cr.set_line_width(1.6);
    cr.set_source_rgb(STROKE.0, STROKE.1, STROKE.2);
    draw(&cr);
    drop(cr);

    let mut buf = Vec::new();
    surface.write_to_png(&mut buf).expect("falha ao codificar ícone");
    gdk::Texture::from_bytes(&glib::Bytes::from(&buf)).expect("falha ao criar textura do ícone")
}

pub fn select() -> gdk::Texture {
    render(|cr| {
        cr.move_to(5.0, 3.0);
        cr.line_to(5.0, 17.0);
        cr.line_to(9.0, 13.3);
        cr.line_to(12.3, 13.3);
        cr.close_path();
        let _ = cr.fill_preserve();
        let _ = cr.stroke();
    })
}

pub fn crop() -> gdk::Texture {
    render(|cr| {
        cr.move_to(4.0, 8.0);
        cr.line_to(4.0, 4.0);
        cr.line_to(8.0, 4.0);
        cr.move_to(12.0, 4.0);
        cr.line_to(16.0, 4.0);
        cr.line_to(16.0, 8.0);
        cr.move_to(16.0, 12.0);
        cr.line_to(16.0, 16.0);
        cr.line_to(12.0, 16.0);
        cr.move_to(8.0, 16.0);
        cr.line_to(4.0, 16.0);
        cr.line_to(4.0, 12.0);
        let _ = cr.stroke();
    })
}

pub fn line() -> gdk::Texture {
    render(|cr| {
        cr.move_to(4.0, 16.0);
        cr.line_to(16.0, 4.0);
        let _ = cr.stroke();
    })
}

pub fn arrow() -> gdk::Texture {
    render(|cr| {
        cr.move_to(4.0, 16.0);
        cr.line_to(16.0, 4.0);
        let _ = cr.stroke();
        cr.move_to(16.0, 4.0);
        cr.line_to(10.7, 5.3);
        let _ = cr.stroke();
        cr.move_to(16.0, 4.0);
        cr.line_to(14.7, 9.3);
        let _ = cr.stroke();
    })
}

pub fn rect() -> gdk::Texture {
    render(|cr| {
        cr.rectangle(4.0, 5.0, 12.0, 10.0);
        let _ = cr.stroke();
    })
}

pub fn ellipse() -> gdk::Texture {
    render(|cr| {
        let _ = cr.save();
        cr.translate(10.0, 10.0);
        cr.scale(1.0, 0.7);
        cr.arc(0.0, 0.0, 7.0, 0.0, std::f64::consts::TAU);
        let _ = cr.restore();
        let _ = cr.stroke();
    })
}

pub fn text() -> gdk::Texture {
    render(|cr| {
        cr.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        cr.set_font_size(14.0);
        cr.move_to(6.0, 15.0);
        let _ = cr.show_text("T");
    })
}

pub fn undo() -> gdk::Texture {
    render(|cr| {
        let _ = cr.save();
        cr.translate(10.0, 11.0);
        cr.arc_negative(0.0, 0.0, 6.0, 0.4, std::f64::consts::PI + 0.6);
        let _ = cr.restore();
        let _ = cr.stroke();
        cr.move_to(4.3, 5.9);
        cr.line_to(1.9, 6.7);
        let _ = cr.stroke();
        cr.move_to(4.3, 5.9);
        cr.line_to(5.0, 3.4);
        let _ = cr.stroke();
    })
}

pub fn cancel() -> gdk::Texture {
    render(|cr| {
        cr.move_to(5.0, 5.0);
        cr.line_to(15.0, 15.0);
        let _ = cr.stroke();
        cr.move_to(15.0, 5.0);
        cr.line_to(5.0, 15.0);
        let _ = cr.stroke();
    })
}

pub fn copy() -> gdk::Texture {
    render(|cr| {
        cr.rectangle(4.0, 6.0, 10.0, 10.0);
        let _ = cr.stroke();
        cr.rectangle(7.0, 3.0, 10.0, 10.0);
        let _ = cr.stroke();
    })
}

pub fn save() -> gdk::Texture {
    render(|cr| {
        cr.rectangle(4.0, 3.0, 12.0, 14.0);
        let _ = cr.stroke();
        cr.rectangle(6.5, 3.0, 7.0, 5.0);
        let _ = cr.stroke();
        cr.rectangle(6.0, 11.0, 8.0, 5.0);
        let _ = cr.stroke();
    })
}
