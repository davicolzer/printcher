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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsb_first_reads_bgrx_bytes_as_rgb() {
        // Um pixel 2x1: (B,G,R,X) = (10,20,30,0) e (40,50,60,0).
        let data = [10u8, 20, 30, 0, 40, 50, 60, 0];
        let img = to_rgba_image(&data, 2, 1, false);
        assert_eq!(img.get_pixel(0, 0), &Rgba([30, 20, 10, 255]));
        assert_eq!(img.get_pixel(1, 0), &Rgba([60, 50, 40, 255]));
    }

    #[test]
    fn msb_first_reads_xrgb_bytes_as_rgb() {
        // Um pixel 2x1: (X,R,G,B) = (0,30,20,10) e (0,60,50,40).
        let data = [0u8, 30, 20, 10, 0, 60, 50, 40];
        let img = to_rgba_image(&data, 2, 1, true);
        assert_eq!(img.get_pixel(0, 0), &Rgba([30, 20, 10, 255]));
        assert_eq!(img.get_pixel(1, 0), &Rgba([60, 50, 40, 255]));
    }

    #[test]
    fn alpha_is_always_opaque() {
        let data = [1u8, 2, 3, 4];
        let img = to_rgba_image(&data, 1, 1, false);
        assert_eq!(img.get_pixel(0, 0)[3], 255);
    }

    #[test]
    fn maps_rows_and_columns_correctly() {
        // 2x2: garante que o índice linear vira (x, y) certo.
        let data: Vec<u8> = (0..16).collect(); // 4 pixels * 4 bytes
        let img = to_rgba_image(&data, 2, 2, false);
        assert_eq!(img.width(), 2);
        assert_eq!(img.height(), 2);
        // pixel (1,1) é o 4º da lista (índice 3), bytes 12..16 -> B=12,G=13,R=14
        assert_eq!(img.get_pixel(1, 1), &Rgba([14, 13, 12, 255]));
    }

    #[test]
    fn ignores_trailing_bytes_beyond_declared_dimensions() {
        // Mais bytes do que width*height precisam -- não deve entrar em pânico
        // nem estourar limites da imagem.
        let data = vec![0u8; 4 * 10];
        let img = to_rgba_image(&data, 2, 2, false);
        assert_eq!(img.width(), 2);
        assert_eq!(img.height(), 2);
    }
}
