//! Lógica pura do editor: estado, geometria, desenho e composição da
//! imagem final. Nada aqui depende de uma janela GTK de verdade — só de
//! `cairo`, que renderiza em software e funciona sem display (é por isso
//! que esse módulo, ao contrário de `editor.rs`, tem testes automatizados
//! de verdade).

use std::fs::File;
use std::path::PathBuf;

use gtk::cairo;

pub(super) type Point = (f64, f64);
pub(super) type Color = (f64, f64, f64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Tool {
    Select,
    Crop,
    Line,
    Arrow,
    Rect,
    Ellipse,
    Text,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CropRect {
    pub(super) p0: Point,
    pub(super) p1: Point,
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
pub(super) enum Annotation {
    Line { p0: Point, p1: Point, color: Color, width: f64 },
    Arrow { p0: Point, p1: Point, color: Color, width: f64 },
    Rect { p0: Point, p1: Point, color: Color, width: f64 },
    Ellipse { p0: Point, p1: Point, color: Color, width: f64 },
    Text { pos: Point, text: String, color: Color, size: f64 },
}

pub(super) struct AppState {
    pub(super) image: cairo::ImageSurface,
    pub(super) image_path: PathBuf,
    pub(super) tool: Tool,
    pub(super) color: Color,
    pub(super) stroke_width: f64,
    pub(super) annotations: Vec<Annotation>,
    pub(super) crop: Option<CropRect>,
    pub(super) drag_start: Option<Point>,
    pub(super) drag_current: Option<Point>,
}

pub(super) fn undo(state: &mut AppState) {
    if state.annotations.pop().is_none() {
        state.crop = None;
    }
}

pub(super) fn make_annotation(tool: Tool, p0: Point, p1: Point, color: Color, width: f64) -> Option<Annotation> {
    match tool {
        Tool::Line => Some(Annotation::Line { p0, p1, color, width }),
        Tool::Arrow => Some(Annotation::Arrow { p0, p1, color, width }),
        Tool::Rect => Some(Annotation::Rect { p0, p1, color, width }),
        Tool::Ellipse => Some(Annotation::Ellipse { p0, p1, color, width }),
        _ => None,
    }
}

pub(super) fn draw_annotation(cr: &cairo::Context, ann: &Annotation) -> Result<(), cairo::Error> {
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

pub(super) fn draw_crop_overlay(cr: &cairo::Context, rect: &CropRect, img_w: f64, img_h: f64) {
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
pub(super) fn compose_final(state: &AppState) -> anyhow::Result<cairo::ImageSurface> {
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

pub(super) fn save_surface(surface: &cairo::ImageSurface, path: &PathBuf) -> anyhow::Result<()> {
    let mut file = File::create(path)?;
    surface
        .write_to_png(&mut file)
        .map_err(|e| anyhow::anyhow!("falha ao gravar PNG: {e:?}"))?;
    Ok(())
}

pub(super) fn encode_png_bytes(surface: &cairo::ImageSurface) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    surface
        .write_to_png(&mut buf)
        .map_err(|e| anyhow::anyhow!("falha ao codificar PNG: {e:?}"))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cria uma superfície de teste preenchida com uma cor sólida. Cairo
    /// renderiza em software, sem precisar de display/GPU — roda igual num
    /// terminal sem sessão gráfica.
    fn solid_surface(w: i32, h: i32, color: Color) -> cairo::ImageSurface {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        cr.set_source_rgb(color.0, color.1, color.2);
        cr.paint().unwrap();
        drop(cr);
        surface
    }

    fn blank_state(image: cairo::ImageSurface) -> AppState {
        AppState {
            image,
            image_path: PathBuf::from("/tmp/printcher-test.png"),
            tool: Tool::Select,
            color: (1.0, 0.0, 0.0),
            stroke_width: 6.0,
            annotations: Vec::new(),
            crop: None,
            drag_start: None,
            drag_current: None,
        }
    }

    /// Decodifica o PNG de saída e devolve o pixel em (x, y), pra
    /// verificações independentes do layout de bytes interno do cairo.
    fn decode_pixel(surface: &cairo::ImageSurface, x: u32, y: u32) -> image::Rgba<u8> {
        let bytes = encode_png_bytes(surface).unwrap();
        let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
        *img.get_pixel(x, y)
    }

    fn close_to(a: u8, b: u8, tolerance: u8) -> bool {
        a.abs_diff(b) <= tolerance
    }

    #[test]
    fn crop_rect_normalizes_points_in_any_order() {
        let rect = CropRect {
            p0: (10.0, 8.0),
            p1: (2.0, 20.0),
        };
        assert_eq!(rect.normalized(), (2.0, 8.0, 10.0, 20.0));

        let already_ordered = CropRect {
            p0: (1.0, 1.0),
            p1: (5.0, 5.0),
        };
        assert_eq!(already_ordered.normalized(), (1.0, 1.0, 5.0, 5.0));
    }

    #[test]
    fn make_annotation_returns_the_matching_shape_for_drawing_tools() {
        let p0 = (0.0, 0.0);
        let p1 = (1.0, 1.0);
        let color = (1.0, 0.0, 0.0);

        assert!(matches!(make_annotation(Tool::Line, p0, p1, color, 1.0), Some(Annotation::Line { .. })));
        assert!(matches!(make_annotation(Tool::Arrow, p0, p1, color, 1.0), Some(Annotation::Arrow { .. })));
        assert!(matches!(make_annotation(Tool::Rect, p0, p1, color, 1.0), Some(Annotation::Rect { .. })));
        assert!(matches!(make_annotation(Tool::Ellipse, p0, p1, color, 1.0), Some(Annotation::Ellipse { .. })));
    }

    #[test]
    fn make_annotation_returns_none_for_non_shape_tools() {
        let p0 = (0.0, 0.0);
        let p1 = (1.0, 1.0);
        let color = (1.0, 0.0, 0.0);

        assert!(make_annotation(Tool::Select, p0, p1, color, 1.0).is_none());
        assert!(make_annotation(Tool::Crop, p0, p1, color, 1.0).is_none());
        assert!(make_annotation(Tool::Text, p0, p1, color, 1.0).is_none());
    }

    #[test]
    fn draw_annotation_succeeds_for_every_variant() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 20, 20).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        let p0 = (2.0, 2.0);
        let p1 = (15.0, 15.0);
        let color = (0.2, 0.4, 0.6);

        for ann in [
            Annotation::Line { p0, p1, color, width: 2.0 },
            Annotation::Arrow { p0, p1, color, width: 2.0 },
            Annotation::Rect { p0, p1, color, width: 2.0 },
            Annotation::Ellipse { p0, p1, color, width: 2.0 },
            Annotation::Text { pos: p0, text: "oi".into(), color, size: 12.0 },
        ] {
            assert!(draw_annotation(&cr, &ann).is_ok());
        }
    }

    #[test]
    fn draw_crop_overlay_does_not_panic_for_any_rect() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 20, 20).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();

        // Ordem normal e invertida -- normalized() já deve resolver os dois.
        draw_crop_overlay(&cr, &CropRect { p0: (5.0, 5.0), p1: (15.0, 15.0) }, 20.0, 20.0);
        draw_crop_overlay(&cr, &CropRect { p0: (15.0, 15.0), p1: (5.0, 5.0) }, 20.0, 20.0);
    }

    #[test]
    fn save_surface_writes_a_readable_png_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("out.png");
        let surface = solid_surface(6, 4, (0.0, 1.0, 0.0));

        save_surface(&surface, &path).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!((img.width(), img.height()), (6, 4));
    }

    #[test]
    fn undo_pops_the_last_annotation_first() {
        let mut state = blank_state(solid_surface(4, 4, (1.0, 1.0, 1.0)));
        state.annotations.push(Annotation::Line {
            p0: (0.0, 0.0),
            p1: (1.0, 1.0),
            color: (1.0, 0.0, 0.0),
            width: 1.0,
        });
        state.crop = Some(CropRect { p0: (0.0, 0.0), p1: (2.0, 2.0) });

        undo(&mut state);
        assert!(state.annotations.is_empty());
        assert!(state.crop.is_some(), "crop não deve ser mexido enquanto houver anotação pra desfazer");

        undo(&mut state);
        assert!(state.crop.is_none(), "sem anotação, undo deve limpar o crop");
    }

    #[test]
    fn compose_final_without_crop_keeps_full_image_size() {
        let state = blank_state(solid_surface(8, 6, (1.0, 1.0, 1.0)));
        let surface = compose_final(&state).unwrap();
        assert_eq!((surface.width(), surface.height()), (8, 6));
    }

    #[test]
    fn compose_final_with_crop_outputs_only_the_cropped_region() {
        let mut state = blank_state(solid_surface(20, 20, (1.0, 1.0, 1.0)));
        state.crop = Some(CropRect {
            p0: (2.0, 1.0),
            p1: (6.0, 4.0),
        });
        let surface = compose_final(&state).unwrap();
        assert_eq!((surface.width(), surface.height()), (4, 3));
    }

    #[test]
    fn compose_final_clamps_crop_to_image_bounds() {
        let mut state = blank_state(solid_surface(10, 10, (1.0, 1.0, 1.0)));
        state.crop = Some(CropRect {
            p0: (-50.0, -50.0),
            p1: (500.0, 500.0),
        });
        let surface = compose_final(&state).unwrap();
        assert_eq!((surface.width(), surface.height()), (10, 10));
    }

    #[test]
    fn compose_final_bakes_annotations_into_the_output_pixels() {
        let mut state = blank_state(solid_surface(20, 20, (1.0, 1.0, 1.0)));
        state.annotations.push(Annotation::Line {
            p0: (0.0, 10.0),
            p1: (20.0, 10.0),
            color: (1.0, 0.0, 0.0),
            width: 8.0,
        });

        let surface = compose_final(&state).unwrap();

        let on_the_line = decode_pixel(&surface, 10, 10);
        assert!(
            on_the_line[0] > 180 && close_to(on_the_line[1], 0, 40) && close_to(on_the_line[2], 0, 40),
            "pixel sobre a linha deveria estar avermelhado, veio {on_the_line:?}"
        );

        let away_from_line = decode_pixel(&surface, 10, 1);
        assert!(
            away_from_line[0] > 240 && away_from_line[1] > 240 && away_from_line[2] > 240,
            "pixel longe da linha deveria continuar branco, veio {away_from_line:?}"
        );
    }
}
