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
    Crop,
    Arrow,
    Rect,
    Ellipse,
    Pen,
    Blur,
    Step,
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
    /// O corte agora é só mais uma operação na mesma pilha de
    /// desfazer/refazer que as demais anotações -- reflete o design (onde
    /// desfazer depois de cortar volta a mostrar a imagem inteira) e
    /// substitui o antigo campo `AppState::crop`, separado.
    Crop(CropRect),
    Arrow { p0: Point, p1: Point, color: Color, width: f64 },
    Rect { p0: Point, p1: Point, color: Color, width: f64 },
    Ellipse { p0: Point, p1: Point, color: Color, width: f64 },
    Pen { points: Vec<Point>, color: Color, width: f64 },
    /// Assim como o corte, não sabe se desenhar sozinho -- precisa da
    /// imagem base pra amostrar os pixels que vai borrar (`draw_blur_region`).
    Blur(CropRect),
    Step { pos: Point, n: u32, color: Color },
}

pub(super) struct AppState {
    pub(super) image: cairo::ImageSurface,
    pub(super) tool: Tool,
    pub(super) color: Color,
    pub(super) stroke_width: f64,
    pub(super) annotations: Vec<Annotation>,
    pub(super) redo_stack: Vec<Annotation>,
    pub(super) drag_start: Option<Point>,
    pub(super) drag_current: Option<Point>,
    /// Pontos acumulados durante um arrasto com a Caneta -- as demais
    /// ferramentas de arrasto só precisam de início/fim
    /// (`drag_start`/`drag_current`), mas a caneta precisa do traço inteiro.
    pub(super) drag_points: Vec<Point>,
    /// Próximo número a usar na ferramenta de Numeração; desfazer/refazer
    /// um `Annotation::Step` também desfaz/refaz esse contador (ver `undo`/
    /// `redo`), pra sempre refletir o que está de fato na tela.
    pub(super) step_n: u32,
    /// Posição do cursor em espaço de imagem, atualizada mesmo sem nenhum
    /// botão pressionado -- só usada pra a lupa (só aparece com a
    /// ferramenta de Cortar ativa).
    pub(super) cursor: Option<Point>,
}

/// Adiciona uma anotação nova (incluindo corte) e limpa a pilha de refazer
/// -- qualquer operação nova invalida o que dava pra refazer antes dela,
/// igual em qualquer editor com desfazer/refazer de verdade.
pub(super) fn push_annotation(state: &mut AppState, ann: Annotation) {
    state.annotations.push(ann);
    state.redo_stack.clear();
}

pub(super) fn undo(state: &mut AppState) {
    if let Some(ann) = state.annotations.pop() {
        if matches!(ann, Annotation::Step { .. }) {
            state.step_n = state.step_n.saturating_sub(1).max(1);
        }
        state.redo_stack.push(ann);
    }
}

pub(super) fn redo(state: &mut AppState) {
    if let Some(ann) = state.redo_stack.pop() {
        if matches!(ann, Annotation::Step { .. }) {
            state.step_n += 1;
        }
        state.annotations.push(ann);
    }
}

/// Cria e empurra uma anotação de Numeração, usando o próximo número
/// disponível (`state.step_n`) e incrementando o contador em seguida --
/// centraliza essa regra num só lugar, já que `undo`/`redo` também
/// precisam saber mexer nesse contador quando desfazem/refazem esse tipo
/// específico de anotação.
pub(super) fn push_step(state: &mut AppState, pos: Point, color: Color) {
    let n = state.step_n;
    push_annotation(state, Annotation::Step { pos, n, color });
    state.step_n += 1;
}

/// O corte "confirmado" é o último `Annotation::Crop` na pilha -- se o
/// usuário cortar de novo depois de já ter cortado, o corte mais recente
/// vale (igual ao protótipo, que faz `ops.filter(kind==='crop').pop()`).
pub(super) fn confirmed_crop(state: &AppState) -> Option<CropRect> {
    state.annotations.iter().rev().find_map(|a| match a {
        Annotation::Crop(r) => Some(*r),
        _ => None,
    })
}

pub(super) fn make_annotation(tool: Tool, p0: Point, p1: Point, color: Color, width: f64) -> Option<Annotation> {
    match tool {
        Tool::Arrow => Some(Annotation::Arrow { p0, p1, color, width }),
        Tool::Rect => Some(Annotation::Rect { p0, p1, color, width }),
        Tool::Ellipse => Some(Annotation::Ellipse { p0, p1, color, width }),
        _ => None,
    }
}

pub(super) fn draw_annotation(cr: &cairo::Context, ann: &Annotation) -> Result<(), cairo::Error> {
    match ann {
        // O corte não é desenhado como uma marca por cima da imagem -- ele
        // é usado só pra recortar o resultado final (`compose_final`) e pra
        // desenhar o overlay tracejado (`draw_crop_overlay`, chamado à
        // parte em editor.rs). Precisa aparecer aqui só porque faz parte da
        // mesma pilha de anotações (desfazer/refazer).
        Annotation::Crop(_) => {}
        // O desfoque precisa da imagem base pra amostrar os pixels -- é
        // tratado à parte em `draw_all_annotations`/`draw_blur_region`.
        Annotation::Blur(_) => {}
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
        Annotation::Pen { points, color, width } => {
            if let Some((first, rest)) = points.split_first() {
                cr.set_source_rgb(color.0, color.1, color.2);
                cr.set_line_width(*width);
                cr.move_to(first.0, first.1);
                for p in rest {
                    cr.line_to(p.0, p.1);
                }
                cr.stroke()?;
            }
        }
        Annotation::Step { pos, n, color } => {
            let radius = 20.0;
            cr.set_source_rgb(color.0, color.1, color.2);
            cr.arc(pos.0, pos.1, radius, 0.0, std::f64::consts::TAU);
            cr.fill()?;

            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size(19.0);
            let label = n.to_string();
            let extents = cr.text_extents(&label)?;
            cr.move_to(pos.0 - extents.width() / 2.0 - extents.x_bearing(), pos.1 - extents.height() / 2.0 - extents.y_bearing());
            cr.show_text(&label)?;
        }
    }
    Ok(())
}

/// Recorta da imagem base a região do retângulo, borra
/// (`box_blur`) e pinta de volta só ali -- é como a ferramenta de Desfoque
/// funciona de verdade (o protótipo usa um `backdrop-filter` de CSS, que
/// não existe fora de navegador; aqui borra os pixels de verdade).
pub(super) fn draw_blur_region(cr: &cairo::Context, image: &cairo::ImageSurface, rect: &CropRect, radius: usize) -> anyhow::Result<()> {
    let (x0, y0, x1, y1) = rect.normalized();
    let (w, h) = ((x1 - x0).max(1.0) as i32, (y1 - y0).max(1.0) as i32);

    let mut region = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)
        .map_err(|e| anyhow::anyhow!("falha ao criar região do desfoque: {e:?}"))?;
    {
        let region_cr = cairo::Context::new(&region).map_err(|e| anyhow::anyhow!("falha ao criar contexto do desfoque: {e:?}"))?;
        region_cr.translate(-x0, -y0);
        region_cr
            .set_source_surface(image, 0.0, 0.0)
            .map_err(|e| anyhow::anyhow!("falha ao recortar região do desfoque: {e:?}"))?;
        region_cr.paint().map_err(|e| anyhow::anyhow!("falha ao pintar região do desfoque: {e:?}"))?;
    }
    let blurred = box_blur(&mut region, radius)?;

    cr.save().map_err(|e| anyhow::anyhow!("{e:?}"))?;
    cr.rectangle(x0, y0, x1 - x0, y1 - y0);
    cr.clip();
    cr.set_source_surface(&blurred, x0, y0)
        .map_err(|e| anyhow::anyhow!("falha ao desenhar desfoque: {e:?}"))?;
    cr.paint().map_err(|e| anyhow::anyhow!("falha ao pintar desfoque: {e:?}"))?;
    cr.restore().map_err(|e| anyhow::anyhow!("{e:?}"))?;
    Ok(())
}

/// Desenha todas as anotações confirmadas, na ordem em que foram feitas --
/// consolida num só lugar (usado por `compose_final` e pelo `draw_func` do
/// editor) o laço que trata `Crop` (não desenha nada) e `Blur` (precisa da
/// imagem base) como casos especiais, mantendo a ordem de empilhamento
/// certa entre os tipos.
pub(super) fn draw_all_annotations(cr: &cairo::Context, state: &AppState) {
    for ann in &state.annotations {
        match ann {
            Annotation::Crop(_) => {}
            Annotation::Blur(rect) => {
                let _ = draw_blur_region(cr, &state.image, rect, BLUR_RADIUS);
            }
            other => {
                let _ = draw_annotation(cr, other);
            }
        }
    }
}

/// Raio do desfoque (em pixels de imagem) -- mesmo valor usado tanto na
/// prévia ao vivo quanto no resultado final, pra não haver diferença
/// visual entre o que o usuário vê arrastando e o que sai salvo.
pub(super) const BLUR_RADIUS: usize = 10;

/// Etiqueta "largura × altura" acima da seleção de corte, em espaço de
/// imagem -- fica 26px acima da borda superior da seleção.
pub(super) fn draw_crop_label(cr: &cairo::Context, rect: &CropRect, img_w: f64, img_h: f64) {
    let (x0, y0, x1, y1) = rect.normalized();
    let label = format!("{} × {}", (x1 - x0).round() as i64, (y1 - y0).round() as i64);

    cr.select_font_face("monospace", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    cr.set_font_size(13.0);
    let Ok(extents) = cr.text_extents(&label) else { return };
    let pad_x = 7.0;
    let pad_y = 4.0;
    let box_w = extents.width() + pad_x * 2.0;
    let box_h = extents.height() + pad_y * 2.0;
    let box_x = x0.clamp(0.0, (img_w - box_w).max(0.0));
    let box_y = (y0 - 26.0).max(0.0).min((img_h - box_h).max(0.0));

    cr.set_source_rgba(14.0 / 255.0, 8.0 / 255.0, 14.0 / 255.0, 0.8);
    cr.rectangle(box_x, box_y, box_w, box_h);
    let _ = cr.fill();

    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.move_to(box_x + pad_x - extents.x_bearing(), box_y + pad_y - extents.y_bearing());
    let _ = cr.show_text(&label);
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
    let (cx0, cy0, cx1, cy1) = match confirmed_crop(state) {
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
    draw_all_annotations(&cr, state);
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

/// Como `save_surface`, mas no formato escolhido pelo usuário
/// (`Config::format`) -- cairo só sabe escrever PNG nativamente.
pub(super) fn save_surface_as(surface: &cairo::ImageSurface, path: &PathBuf, format: crate::config::ImageFormat) -> anyhow::Result<()> {
    use crate::config::ImageFormat;
    use image::ImageEncoder;

    if format == ImageFormat::Png {
        return save_surface(surface, path);
    }

    // Pixels do cairo são ARGB32 pré-multiplicado, em ordem de bytes
    // nativa -- em vez de reimplementar a matemática de
    // des-pré-multiplicar alpha na mão, reaproveita o próprio codificador
    // PNG do cairo (que já faz essa conversão certinho) e decodifica de
    // volta com a crate `image`, que os codificadores de JPG/WebP entendem.
    let png_bytes = encode_png_bytes(surface)?;
    let img = image::load_from_memory(&png_bytes)
        .map_err(|e| anyhow::anyhow!("falha ao decodificar imagem pra conversão: {e}"))?
        .to_rgba8();

    let mut file = File::create(path)?;
    match format {
        ImageFormat::Png => unreachable!(),
        ImageFormat::Jpg => {
            // JPEG não tem canal alfa -- compõe sobre fundo branco antes.
            let mut rgb = image::RgbImage::new(img.width(), img.height());
            for (dst, src) in rgb.pixels_mut().zip(img.pixels()) {
                let a = src.0[3] as f32 / 255.0;
                for c in 0..3 {
                    dst.0[c] = (src.0[c] as f32 * a + 255.0 * (1.0 - a)) as u8;
                }
            }
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 90);
            encoder
                .write_image(rgb.as_raw(), rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
                .map_err(|e| anyhow::anyhow!("falha ao codificar JPG: {e}"))?;
        }
        ImageFormat::WebP => {
            let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut file);
            encoder
                .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8)
                .map_err(|e| anyhow::anyhow!("falha ao codificar WebP: {e}"))?;
        }
    }
    Ok(())
}

/// Borrão simples (box blur separável, horizontal e depois vertical) sobre
/// uma cópia da superfície -- usado pra imitar o efeito de vidro fosco
/// atrás da barra flutuante do editor. Wayland não tem "backdrop-filter"
/// de verdade pra janelas comuns (isso é recurso de compositor, não do
/// Mutter/GNOME); como o fundo por trás da barra é sempre a captura
/// congelada (uma bitmap estática que já temos em memória, não o desktop
/// ao vivo), dá pra borrar a região na mão em vez de depender do sistema.
pub(super) fn box_blur(surface: &mut cairo::ImageSurface, radius: usize) -> anyhow::Result<cairo::ImageSurface> {
    let width = surface.width();
    let height = surface.height();
    let stride = surface.stride();

    let buf = {
        let data = surface.data().map_err(|e| anyhow::anyhow!("falha ao ler pixels pro blur: {e:?}"))?;
        let mut buf = data.to_vec();
        if radius > 0 {
            box_blur_pass(&mut buf, width as usize, height as usize, stride as usize, radius, true);
            box_blur_pass(&mut buf, width as usize, height as usize, stride as usize, radius, false);
        }
        buf
    };

    let mut out = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)
        .map_err(|e| anyhow::anyhow!("falha ao criar superfície de blur: {e:?}"))?;
    {
        let mut out_data = out.data().map_err(|e| anyhow::anyhow!("falha ao escrever pixels do blur: {e:?}"))?;
        out_data.copy_from_slice(&buf);
    }
    Ok(out)
}

/// Uma passada de box blur (horizontal ou vertical) num buffer ARGB32 de 4
/// bytes por pixel. Cada um dos 4 bytes é borrado de forma independente --
/// não importa a ordem real dos canais (R/G/B/A vs B/G/R/A), o resultado é
/// o mesmo já que todos recebem o mesmo tratamento.
fn box_blur_pass(buf: &mut [u8], width: usize, height: usize, stride: usize, radius: usize, horizontal: bool) {
    let original = buf.to_vec();
    let r = radius as isize;

    for y in 0..height {
        for x in 0..width {
            let mut sums = [0u32; 4];
            let mut count = 0u32;
            for offset in -r..=r {
                let (sx, sy) = if horizontal {
                    (x as isize + offset, y as isize)
                } else {
                    (x as isize, y as isize + offset)
                };
                if sx < 0 || sy < 0 || sx as usize >= width || sy as usize >= height {
                    continue;
                }
                let idx = sy as usize * stride + sx as usize * 4;
                for (c, sum) in sums.iter_mut().enumerate() {
                    *sum += original[idx + c] as u32;
                }
                count += 1;
            }
            let idx = y * stride + x * 4;
            for (c, sum) in sums.iter().enumerate() {
                buf[idx + c] = (*sum / count.max(1)) as u8;
            }
        }
    }
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
            tool: Tool::Crop,
            color: (1.0, 0.0, 0.0),
            stroke_width: 6.0,
            annotations: Vec::new(),
            redo_stack: Vec::new(),
            drag_start: None,
            drag_current: None,
            drag_points: Vec::new(),
            step_n: 1,
            cursor: None,
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

        assert!(matches!(make_annotation(Tool::Arrow, p0, p1, color, 1.0), Some(Annotation::Arrow { .. })));
        assert!(matches!(make_annotation(Tool::Rect, p0, p1, color, 1.0), Some(Annotation::Rect { .. })));
        assert!(matches!(make_annotation(Tool::Ellipse, p0, p1, color, 1.0), Some(Annotation::Ellipse { .. })));
    }

    #[test]
    fn make_annotation_returns_none_for_non_shape_tools() {
        let p0 = (0.0, 0.0);
        let p1 = (1.0, 1.0);
        let color = (1.0, 0.0, 0.0);

        assert!(make_annotation(Tool::Crop, p0, p1, color, 1.0).is_none());
        assert!(make_annotation(Tool::Pen, p0, p1, color, 1.0).is_none());
    }

    #[test]
    fn draw_annotation_succeeds_for_every_variant() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 20, 20).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        let p0 = (2.0, 2.0);
        let p1 = (15.0, 15.0);
        let color = (0.2, 0.4, 0.6);

        for ann in [
            Annotation::Crop(CropRect { p0, p1 }),
            Annotation::Blur(CropRect { p0, p1 }),
            Annotation::Arrow { p0, p1, color, width: 2.0 },
            Annotation::Rect { p0, p1, color, width: 2.0 },
            Annotation::Ellipse { p0, p1, color, width: 2.0 },
            Annotation::Pen { points: vec![p0, (5.0, 8.0), p1], color, width: 2.0 },
            Annotation::Step { pos: p0, n: 3, color },
        ] {
            assert!(draw_annotation(&cr, &ann).is_ok());
        }
    }

    #[test]
    fn draw_annotation_pen_with_no_points_does_not_panic() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 20, 20).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        let ann = Annotation::Pen { points: Vec::new(), color: (1.0, 0.0, 0.0), width: 2.0 };
        assert!(draw_annotation(&cr, &ann).is_ok());
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
    fn save_surface_as_writes_a_readable_file_in_every_format() {
        use crate::config::ImageFormat;

        let tmp = tempfile::tempdir().unwrap();
        let surface = solid_surface(6, 4, (0.0, 1.0, 0.0));

        for format in [ImageFormat::Png, ImageFormat::Jpg, ImageFormat::WebP] {
            let path = tmp.path().join(format!("out.{}", format.extension()));
            save_surface_as(&surface, &path, format).unwrap();

            let bytes = std::fs::read(&path).unwrap();
            let img = image::load_from_memory(&bytes).unwrap();
            assert_eq!((img.width(), img.height()), (6, 4), "formato {format:?} não bateu o tamanho");
        }
    }

    #[test]
    fn undo_moves_the_last_annotation_to_the_redo_stack() {
        let mut state = blank_state(solid_surface(4, 4, (1.0, 1.0, 1.0)));
        push_annotation(&mut state, Annotation::Crop(CropRect { p0: (0.0, 0.0), p1: (2.0, 2.0) }));
        push_annotation(
            &mut state,
            Annotation::Arrow {
                p0: (0.0, 0.0),
                p1: (1.0, 1.0),
                color: (1.0, 0.0, 0.0),
                width: 1.0,
            },
        );

        undo(&mut state);
        assert_eq!(state.annotations.len(), 1, "corte não deve ser mexido enquanto houver anotação pra desfazer");
        assert!(confirmed_crop(&state).is_some());
        assert_eq!(state.redo_stack.len(), 1);

        undo(&mut state);
        assert!(state.annotations.is_empty());
        assert!(confirmed_crop(&state).is_none(), "sem mais anotações, o corte também deve sair");
        assert_eq!(state.redo_stack.len(), 2);
    }

    #[test]
    fn redo_brings_back_the_last_undone_annotation() {
        let mut state = blank_state(solid_surface(4, 4, (1.0, 1.0, 1.0)));
        push_annotation(&mut state, Annotation::Crop(CropRect { p0: (0.0, 0.0), p1: (2.0, 2.0) }));

        undo(&mut state);
        assert!(state.annotations.is_empty());

        redo(&mut state);
        assert!(confirmed_crop(&state).is_some());
        assert!(state.redo_stack.is_empty());
    }

    #[test]
    fn pushing_a_new_annotation_clears_the_redo_stack() {
        let mut state = blank_state(solid_surface(4, 4, (1.0, 1.0, 1.0)));
        push_annotation(&mut state, Annotation::Crop(CropRect { p0: (0.0, 0.0), p1: (2.0, 2.0) }));
        undo(&mut state);
        assert_eq!(state.redo_stack.len(), 1);

        push_annotation(
            &mut state,
            Annotation::Rect {
                p0: (0.0, 0.0),
                p1: (1.0, 1.0),
                color: (1.0, 0.0, 0.0),
                width: 1.0,
            },
        );
        assert!(state.redo_stack.is_empty(), "uma operação nova deve invalidar o que dava pra refazer");
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
        push_annotation(&mut state, Annotation::Crop(CropRect { p0: (2.0, 1.0), p1: (6.0, 4.0) }));
        let surface = compose_final(&state).unwrap();
        assert_eq!((surface.width(), surface.height()), (4, 3));
    }

    #[test]
    fn compose_final_clamps_crop_to_image_bounds() {
        let mut state = blank_state(solid_surface(10, 10, (1.0, 1.0, 1.0)));
        push_annotation(&mut state, Annotation::Crop(CropRect { p0: (-50.0, -50.0), p1: (500.0, 500.0) }));
        let surface = compose_final(&state).unwrap();
        assert_eq!((surface.width(), surface.height()), (10, 10));
    }

    #[test]
    fn compose_final_bakes_annotations_into_the_output_pixels() {
        let mut state = blank_state(solid_surface(20, 20, (1.0, 1.0, 1.0)));
        push_annotation(
            &mut state,
            Annotation::Rect {
                p0: (0.0, 6.0),
                p1: (20.0, 14.0),
                color: (1.0, 0.0, 0.0),
                width: 2.0,
            },
        );

        let surface = compose_final(&state).unwrap();

        let on_the_border = decode_pixel(&surface, 10, 6);
        assert!(
            on_the_border[0] > 180 && close_to(on_the_border[1], 0, 40) && close_to(on_the_border[2], 0, 40),
            "pixel sobre a borda do retângulo deveria estar avermelhado, veio {on_the_border:?}"
        );

        let away_from_border = decode_pixel(&surface, 10, 1);
        assert!(
            away_from_border[0] > 240 && away_from_border[1] > 240 && away_from_border[2] > 240,
            "pixel longe da borda deveria continuar branco, veio {away_from_border:?}"
        );
    }

    #[test]
    fn box_blur_keeps_a_uniform_color_unchanged() {
        let mut surface = solid_surface(30, 30, (0.4, 0.6, 0.8));
        let blurred = box_blur(&mut surface, 4).unwrap();

        // Longe das bordas, borrar uma cor uniforme não deveria mudar nada
        // (a média de vizinhos com a mesma cor é a própria cor).
        let pixel = decode_pixel(&blurred, 15, 15);
        assert!(close_to(pixel[0], 102, 3) && close_to(pixel[1], 153, 3) && close_to(pixel[2], 204, 3), "veio {pixel:?}");
    }

    #[test]
    fn box_blur_softens_a_sharp_edge() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 30, 30).unwrap();
        {
            let cr = cairo::Context::new(&surface).unwrap();
            cr.set_source_rgb(0.0, 0.0, 0.0);
            cr.rectangle(0.0, 0.0, 15.0, 30.0);
            cr.fill().unwrap();
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.rectangle(15.0, 0.0, 15.0, 30.0);
            cr.fill().unwrap();
        }
        let mut surface = surface;
        let blurred = box_blur(&mut surface, 6).unwrap();

        // Bem na fronteira entre preto e branco, o resultado borrado deve
        // ficar em algum tom de cinza intermediário -- nem preto nem branco
        // puro, diferente da imagem original nesse mesmo ponto.
        let pixel = decode_pixel(&blurred, 15, 15);
        assert!(pixel[0] > 20 && pixel[0] < 235, "esperava um cinza intermediário na borda, veio {pixel:?}");
    }

    #[test]
    fn draw_blur_region_softens_only_the_given_rect() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 30, 30).unwrap();
        {
            let cr = cairo::Context::new(&surface).unwrap();
            cr.set_source_rgb(0.0, 0.0, 0.0);
            cr.rectangle(0.0, 0.0, 15.0, 30.0);
            cr.fill().unwrap();
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.rectangle(15.0, 0.0, 15.0, 30.0);
            cr.fill().unwrap();
        }

        let out = cairo::ImageSurface::create(cairo::Format::ARgb32, 30, 30).unwrap();
        let out_cr = cairo::Context::new(&out).unwrap();
        out_cr.set_source_surface(&surface, 0.0, 0.0).unwrap();
        out_cr.paint().unwrap();
        draw_blur_region(&out_cr, &surface, &CropRect { p0: (10.0, 0.0), p1: (20.0, 30.0) }, 6).unwrap();
        drop(out_cr);

        // Dentro do retângulo borrado, a fronteira preto/branco vira cinza.
        let inside = decode_pixel(&out, 15, 15);
        assert!(inside[0] > 20 && inside[0] < 235, "esperava cinza dentro da região borrada, veio {inside:?}");

        // Longe do retângulo, a imagem continua nítida (preto/branco puro).
        let outside = decode_pixel(&out, 2, 15);
        assert!(outside[0] < 10, "fora da região, devia continuar preto puro, veio {outside:?}");
    }

    #[test]
    fn push_step_assigns_sequential_numbers_and_undo_redo_track_the_counter() {
        let mut state = blank_state(solid_surface(20, 20, (1.0, 1.0, 1.0)));
        assert_eq!(state.step_n, 1);

        push_step(&mut state, (5.0, 5.0), (1.0, 0.0, 0.0));
        push_step(&mut state, (8.0, 8.0), (1.0, 0.0, 0.0));
        assert_eq!(state.step_n, 3);
        assert!(matches!(state.annotations[0], Annotation::Step { n: 1, .. }));
        assert!(matches!(state.annotations[1], Annotation::Step { n: 2, .. }));

        undo(&mut state);
        assert_eq!(state.step_n, 2, "desfazer o 2º passo deve liberar o número 2 de novo");

        redo(&mut state);
        assert_eq!(state.step_n, 3);
    }

    #[test]
    fn draw_all_annotations_skips_crop_and_blurs_in_place_without_touching_shape_order() {
        let mut state = blank_state(solid_surface(20, 20, (0.0, 0.0, 0.0)));
        push_annotation(&mut state, Annotation::Crop(CropRect { p0: (0.0, 0.0), p1: (20.0, 20.0) }));
        push_annotation(
            &mut state,
            Annotation::Rect {
                p0: (0.0, 0.0),
                p1: (20.0, 20.0),
                color: (1.0, 1.0, 1.0),
                width: 20.0,
            },
        );

        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 20, 20).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        // Não deve entrar em pânico nem com Crop (não desenha nada) nem
        // com anotações vetoriais normais no mesmo laço.
        draw_all_annotations(&cr, &state);
    }
}
