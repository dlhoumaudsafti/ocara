// Transformations sur place : redimensionnement, recadrage, rotation, miroirs.

use image::imageops::{self, FilterType};

use crate::raster::{rotate_any, rotated_size};
use crate::*;

/// img.resize(width, height), filtre de Lanczos.
#[unsafe(no_mangle)]
pub extern "C" fn Image_resize(this: i64, width: i64, height: i64) {
    with_canvas(this, |c| {
        let (w, h) = dimensions(width, height)?;
        c.pixels = imageops::resize(&c.pixels, w, h, FilterType::Lanczos3);
        Ok(())
    });
}

/// img.crop(x, y, w, h) : ne garde que ce rectangle, contenu dans l'image.
#[unsafe(no_mangle)]
pub extern "C" fn Image_crop(this: i64, x: i64, y: i64, w: i64, h: i64) {
    with_canvas(this, |c| {
        let (cw, ch) = dimensions(w, h)?;
        let fits = x >= 0 && y >= 0
            && x as i128 + w as i128 <= c.pixels.width() as i128
            && y as i128 + h as i128 <= c.pixels.height() as i128;
        if !fits {
            return Err(Failure::new(ERR_BOUNDS, format!(
                "rectangle ({x}, {y}, {w}x{h}) hors de l'image {}x{}", c.pixels.width(), c.pixels.height(),
            )));
        }
        c.pixels = imageops::crop_imm(&c.pixels, x as u32, y as u32, cw, ch).to_image();
        Ok(())
    });
}

/// img.rotate(degrees), sens horaire. Un multiple de 90° est exact ; un
/// autre angle agrandit l'image pour tout contenir, fond transparent.
#[unsafe(no_mangle)]
pub extern "C" fn Image_rotate(this: i64, degrees: f64) {
    with_canvas(this, |c| {
        if !degrees.is_finite() {
            return Err(Failure::new(ERR_VALUE, format!("angle invalide : {degrees}")));
        }
        let angle = degrees.rem_euclid(360.0);
        c.pixels = if angle == 0.0 {
            return Ok(());
        } else if angle == 90.0 {
            imageops::rotate90(&c.pixels)
        } else if angle == 180.0 {
            imageops::rotate180(&c.pixels)
        } else if angle == 270.0 {
            imageops::rotate270(&c.pixels)
        } else {
            let (w, h) = rotated_size(&c.pixels, angle);
            rotate_any(&c.pixels, angle, dimensions(w, h)?)
        };
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn Image_flipH(this: i64) {
    with_canvas(this, |c| {
        imageops::flip_horizontal_in_place(&mut c.pixels);
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn Image_flipV(this: i64) {
    with_canvas(this, |c| {
        imageops::flip_vertical_in_place(&mut c.pixels);
        Ok(())
    });
}
