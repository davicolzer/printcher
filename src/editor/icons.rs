//! Ícones do editor, desenhados via Cairo em vez de carregados de um tema de
//! ícones do sistema -- garante aparência consistente e independe de quais
//! ícones simbólicos estão instalados (dev machine vs runtime do Flatpak).
//!
//! As coordenadas de cada ícone são as do design (`design_handoff_printcher/`),
//! convertidas do viewBox `0 0 24 24` do SVG original pra este canvas de
//! 20×20 (fator ~0.833), pra reproduzir os traços com fidelidade.
//!
//! Cada função recebe a cor do traço em vez de usar uma cor fixa: ao
//! contrário de um ícone de tema (que o GTK sabe recolorir sozinho via
//! `currentColor`), esse é um bitmap já pronto -- a única forma de mudar de
//! cor conforme o estado do botão (ferramenta ativa/inativa, por exemplo) é
//! gerar uma textura nova pra cada cor precisada (ver `editor.rs`, que gera
//! duas versões de cada ícone de ferramenta).

use gtk::cairo;
use gtk::gdk;
use gtk::glib;

const SIZE: i32 = 20;

pub type Color = (f64, f64, f64, f64);

fn render(color: Color, draw: impl Fn(&cairo::Context)) -> gdk::Texture {
    let surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, SIZE, SIZE).expect("falha ao criar superfície do ícone");
    let cr = cairo::Context::new(&surface).expect("falha ao criar contexto do ícone");
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    cr.set_line_width(1.6);
    cr.set_source_rgba(color.0, color.1, color.2, color.3);
    draw(&cr);
    drop(cr);

    let mut buf = Vec::new();
    surface.write_to_png(&mut buf).expect("falha ao codificar ícone");
    gdk::Texture::from_bytes(&glib::Bytes::from(&buf)).expect("falha ao criar textura do ícone")
}

pub fn crop(color: Color) -> gdk::Texture {
    render(color, |cr| {
        cr.move_to(5.8, 2.5);
        cr.line_to(5.8, 14.2);
        cr.line_to(17.5, 14.2);
        cr.move_to(14.2, 17.5);
        cr.line_to(14.2, 5.8);
        cr.line_to(2.5, 5.8);
        let _ = cr.stroke();
    })
}

pub fn arrow(color: Color) -> gdk::Texture {
    render(color, |cr| {
        cr.move_to(4.2, 15.8);
        cr.line_to(15.8, 4.2);
        let _ = cr.stroke();
        cr.move_to(15.8, 10.8);
        cr.line_to(15.8, 4.2);
        cr.line_to(9.2, 4.2);
        let _ = cr.stroke();
    })
}

pub fn rect(color: Color) -> gdk::Texture {
    render(color, |cr| {
        cr.rectangle(3.3, 5.0, 13.3, 10.0);
        let _ = cr.stroke();
    })
}

pub fn ellipse(color: Color) -> gdk::Texture {
    render(color, |cr| {
        let _ = cr.save();
        cr.translate(10.0, 10.0);
        cr.scale(7.5, 5.8);
        cr.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
        let _ = cr.restore();
        let _ = cr.stroke();
    })
}

pub fn undo(color: Color) -> gdk::Texture {
    render(color, |cr| {
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

pub fn cancel(color: Color) -> gdk::Texture {
    render(color, |cr| {
        cr.move_to(5.0, 5.0);
        cr.line_to(15.0, 15.0);
        let _ = cr.stroke();
        cr.move_to(15.0, 5.0);
        cr.line_to(5.0, 15.0);
        let _ = cr.stroke();
    })
}

pub fn copy(color: Color) -> gdk::Texture {
    render(color, |cr| {
        cr.rectangle(4.2, 4.2, 8.3, 8.3);
        let _ = cr.stroke();
        cr.rectangle(7.5, 7.5, 9.2, 9.2);
        let _ = cr.stroke();
    })
}

pub fn save(color: Color) -> gdk::Texture {
    render(color, |cr| {
        cr.move_to(10.0, 3.3);
        cr.line_to(10.0, 11.7);
        let _ = cr.stroke();
        cr.move_to(6.7, 9.2);
        cr.line_to(10.0, 12.5);
        cr.line_to(13.3, 9.2);
        let _ = cr.stroke();
        cr.move_to(4.2, 15.0);
        cr.line_to(15.8, 15.0);
        let _ = cr.stroke();
    })
}

pub fn pen(color: Color) -> gdk::Texture {
    render(color, |cr| {
        cr.move_to(3.3, 16.7);
        cr.line_to(6.7, 15.8);
        cr.line_to(15.8, 6.7);
        cr.line_to(13.3, 4.2);
        cr.line_to(4.2, 13.3);
        cr.close_path();
        let _ = cr.stroke();
    })
}

/// Diferente dos outros ícones (contorno), esse é preenchido -- 4
/// quadrados em xadrez, dois deles com opacidade reduzida (igual ao SVG do
/// design), representando pixels desfocados.
pub fn blur(color: Color) -> gdk::Texture {
    render(color, |cr| {
        let dim = (color.0, color.1, color.2, color.3 * 0.55);
        let squares = [(3.3, 3.3, false), (11.7, 3.3, true), (3.3, 11.7, true), (11.7, 11.7, false)];
        for (x, y, faded) in squares {
            if faded {
                cr.set_source_rgba(dim.0, dim.1, dim.2, dim.3);
            } else {
                cr.set_source_rgba(color.0, color.1, color.2, color.3);
            }
            cr.rectangle(x, y, 5.0, 5.0);
            let _ = cr.fill();
        }
    })
}

pub fn step(color: Color) -> gdk::Texture {
    render(color, |cr| {
        cr.arc(10.0, 10.0, 7.08, 0.0, std::f64::consts::TAU);
        let _ = cr.stroke();
        cr.move_to(8.75, 8.0);
        cr.line_to(10.33, 7.17);
        cr.line_to(10.33, 13.33);
        let _ = cr.stroke();
    })
}
