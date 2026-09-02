use std::path::PathBuf;

use image::{Rgba, RgbaImage};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat, ImageOrder};

/// Captura a tela cheia via conexão X11 direta (`GetImage` na janela raiz)
/// e salva o resultado no destino padrão do printcher.
///
/// Assume um visual TrueColor de 24/32 bits, o caso comum em servidores X
/// modernos. Ainda não testado em uma sessão X11 real — só compilado.
pub fn capture_fullscreen() -> anyhow::Result<PathBuf> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let width = screen.width_in_pixels;
    let height = screen.height_in_pixels;

    let reply = conn
        .get_image(ImageFormat::Z_PIXMAP, root, 0, 0, width, height, !0)?
        .reply()?;

    let msb_first = conn.setup().image_byte_order == ImageOrder::MSB_FIRST;
    let rgba = to_rgba_image(&reply.data, width, height, msb_first);

    let dest_path = super::dest_path()?;
    rgba.save(&dest_path)?;

    Ok(dest_path)
}

/// Converte o buffer bruto (formato BGRX/XRGB de 32 bits por pixel,
/// dependendo da ordem de bytes do servidor) em uma imagem RGBA.
fn to_rgba_image(data: &[u8], width: u16, height: u16, msb_first: bool) -> RgbaImage {
    let (w, h) = (width as u32, height as u32);
    let mut img = RgbaImage::new(w, h);
    for (i, px) in data.as_chunks::<4>().0.iter().enumerate() {
        if i as u32 >= w * h {
            break;
        }
        let (r, g, b) = if msb_first {
            (px[1], px[2], px[3])
        } else {
            (px[2], px[1], px[0])
        };
        let x = i as u32 % w;
        let y = i as u32 / w;
        img.put_pixel(x, y, Rgba([r, g, b, 255]));
    }
    img
}
