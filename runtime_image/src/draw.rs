// Pixels et primitives de dessin. Les primitives composent la couleur
// courante (`setDrawColor`) par-dessus l'image (alpha) et ignorent ce qui
// dépasse ; `setPixel` et `clear` remplacent les pixels tels quels.

use image::Rgba;

use crate::ffi::int_array;
use crate::raster::{blend, clip_line, half_width, line, plot, rows, span};
use crate::*;

#[unsafe(no_mangle)]
pub extern "C" fn Image_setDrawColor(this: i64, r: i64, g: i64, b: i64, a: i64) {
    with_canvas(this, |c| {
        c.color = rgba(r, g, b, a)?;
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn Image_setPixel(this: i64, x: i64, y: i64, r: i64, g: i64, b: i64, a: i64) {
    with_canvas(this, |c| {
        let color = rgba(r, g, b, a)?;
        let (x, y) = inside(&c.pixels, x, y)?;
        c.pixels.put_pixel(x, y, color);
        Ok(())
    });
}

/// img.getPixel(x, y) → [r, g, b, a]
#[unsafe(no_mangle)]
pub extern "C" fn Image_getPixel(this: i64, x: i64, y: i64) -> i64 {
    let Rgba(channels) = with_canvas(this, |c| {
        let (x, y) = inside(&c.pixels, x, y)?;
        Ok(*c.pixels.get_pixel(x, y))
    });
    int_array(&channels.map(i64::from))
}

#[unsafe(no_mangle)]
pub extern "C" fn Image_clear(this: i64) {
    with_canvas(this, |c| {
        let color = c.color;
        c.pixels.pixels_mut().for_each(|p| *p = color);
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn Image_drawPoint(this: i64, x: i64, y: i64) {
    with_canvas(this, |c| {
        plot(&mut c.pixels, x as i128, y as i128, c.color);
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn Image_drawLine(this: i64, x1: i64, y1: i64, x2: i64, y2: i64) {
    with_canvas(this, |c| {
        if let Some((from, to)) = clip_line(&c.pixels, (x1 as f64, y1 as f64), (x2 as f64, y2 as f64)) {
            line(&mut c.pixels, from, to, c.color);
        }
        Ok(())
    });
}

fn extent(w: i64, h: i64) -> Result<(i128, i128), Failure> {
    if w < 0 || h < 0 {
        return Err(Failure::new(ERR_SIZE, format!("dimensions invalides : {w}x{h}")));
    }
    Ok((w as i128, h as i128))
}

#[unsafe(no_mangle)]
pub extern "C" fn Image_fillRect(this: i64, x: i64, y: i64, w: i64, h: i64) {
    with_canvas(this, |c| {
        let (w, h) = extent(w, h)?;
        let (x, y) = (x as i128, y as i128);
        for row in rows(&c.pixels, y, y + h - 1) {
            span(&mut c.pixels, row, x, x + w - 1, c.color);
        }
        Ok(())
    });
}

/// Contour d'un pixel d'épaisseur, chaque pixel composé une seule fois.
#[unsafe(no_mangle)]
pub extern "C" fn Image_drawRect(this: i64, x: i64, y: i64, w: i64, h: i64) {
    with_canvas(this, |c| {
        let (w, h) = extent(w, h)?;
        if w == 0 || h == 0 {
            return Ok(());
        }
        let (x, y, color) = (x as i128, y as i128, c.color);
        span(&mut c.pixels, y, x, x + w - 1, color);
        if h > 1 {
            span(&mut c.pixels, y + h - 1, x, x + w - 1, color);
        }
        for row in rows(&c.pixels, y + 1, y + h - 2) {
            plot(&mut c.pixels, x, row, color);
            if w > 1 {
                plot(&mut c.pixels, x + w - 1, row, color);
            }
        }
        Ok(())
    });
}

fn radius_of(radius: i64) -> Result<i128, Failure> {
    if radius < 0 {
        return Err(Failure::new(ERR_SIZE, format!("rayon invalide : {radius}")));
    }
    Ok(radius as i128)
}

#[unsafe(no_mangle)]
pub extern "C" fn Image_fillCircle(this: i64, cx: i64, cy: i64, radius: i64) {
    with_canvas(this, |c| {
        let r = radius_of(radius)?;
        let (cx, cy, outer) = (cx as i128, cy as i128, r as f64 + 0.5);
        for row in rows(&c.pixels, cy - r, cy + r) {
            if let Some(half) = half_width(outer, row - cy) {
                span(&mut c.pixels, row, cx - half, cx + half, c.color);
            }
        }
        Ok(())
    });
}

/// Cercle d'un pixel d'épaisseur : pixels à une distance du centre comprise
/// entre `radius - 0.5` et `radius + 0.5`.
#[unsafe(no_mangle)]
pub extern "C" fn Image_drawCircle(this: i64, cx: i64, cy: i64, radius: i64) {
    with_canvas(this, |c| {
        let r = radius_of(radius)?;
        let (cx, cy, color) = (cx as i128, cy as i128, c.color);
        if r == 0 {
            plot(&mut c.pixels, cx, cy, color);
            return Ok(());
        }
        for row in rows(&c.pixels, cy - r, cy + r) {
            let Some(outer) = half_width(r as f64 + 0.5, row - cy) else { continue };
            match half_width(r as f64 - 0.5, row - cy) {
                Some(inner) => {
                    span(&mut c.pixels, row, cx - outer, cx - inner - 1, color);
                    span(&mut c.pixels, row, cx + inner + 1, cx + outer, color);
                }
                None => span(&mut c.pixels, row, cx - outer, cx + outer, color),
            }
        }
        Ok(())
    });
}

/// img.drawImage(src, x, y) : `src` composée par-dessus l'image, coin
/// supérieur gauche en (x, y).
#[unsafe(no_mangle)]
pub extern "C" fn Image_drawImage(this: i64, src: i64, x: i64, y: i64) {
    let (target, source) = (image_of(this), image_of(src));
    if std::ptr::eq(target, source) {
        let mut canvas = lock(target);
        let copy = canvas.pixels.clone();
        compose(&mut canvas.pixels, &copy, x, y);
        return;
    }
    // Verrous pris dans l'ordre des adresses : deux compositions croisées
    // sur deux threads ne s'attendent jamais mutuellement.
    let (mut dst, src) = if this < src {
        let dst = lock(target);
        (dst, lock(source))
    } else {
        let src = lock(source);
        (lock(target), src)
    };
    compose(&mut dst.pixels, &src.pixels, x, y);
}

fn compose(dst: &mut image::RgbaImage, src: &image::RgbaImage, x: i64, y: i64) {
    let (x, y) = (x as i128, y as i128);
    for row in rows(dst, y, y + src.height() as i128 - 1) {
        let src_y = (row - y) as u32;
        let start = x.max(0);
        let end = (x + src.width() as i128 - 1).min(dst.width() as i128 - 1);
        for col in start..=end {
            blend(dst.get_pixel_mut(col as u32, row as u32), *src.get_pixel((col - x) as u32, src_y));
        }
    }
}
