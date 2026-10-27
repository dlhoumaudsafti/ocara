// Rastérisation : composition alpha d'un pixel, segments horizontaux,
// découpe d'une ligne, rotation libre. Toute coordonnée hors de l'image est
// ignorée ; les calculs se font en i128 pour qu'aucune coordonnée Ocara
// (i64) ne déborde.

use image::{Rgba, RgbaImage};

/// Composition « source over » en alpha non prémultiplié.
pub(crate) fn blend(dst: &mut Rgba<u8>, src: Rgba<u8>) {
    let sa = src[3] as u32;
    if sa == 255 {
        *dst = src;
        return;
    }
    if sa == 0 {
        return;
    }
    let da = dst[3] as u32 * (255 - sa) / 255;
    let oa = sa + da;
    for i in 0..3 {
        dst[i] = ((src[i] as u32 * sa + dst[i] as u32 * da + oa / 2) / oa) as u8;
    }
    dst[3] = oa as u8;
}

pub(crate) fn plot(pixels: &mut RgbaImage, x: i128, y: i128, color: Rgba<u8>) {
    if x >= 0 && y >= 0 && x < pixels.width() as i128 && y < pixels.height() as i128 {
        blend(pixels.get_pixel_mut(x as u32, y as u32), color);
    }
}

/// Pixels `x0..=x1` de la ligne `y`.
pub(crate) fn span(pixels: &mut RgbaImage, y: i128, x0: i128, x1: i128, color: Rgba<u8>) {
    if y < 0 || y >= pixels.height() as i128 {
        return;
    }
    let (start, end) = (x0.max(0), x1.min(pixels.width() as i128 - 1));
    for x in start..=end {
        blend(pixels.get_pixel_mut(x as u32, y as u32), color);
    }
}

/// Lignes de l'image comprises entre `y0` et `y1`.
pub(crate) fn rows(pixels: &RgbaImage, y0: i128, y1: i128) -> std::ops::RangeInclusive<i128> {
    y0.max(0)..=y1.min(pixels.height() as i128 - 1)
}

/// Segment découpé à l'image (Liang–Barsky), extrémités arrondies au pixel.
pub(crate) fn clip_line(pixels: &RgbaImage, from: (f64, f64), to: (f64, f64)) -> Option<((i128, i128), (i128, i128))> {
    let (max_x, max_y) = (pixels.width() as f64 - 1.0, pixels.height() as f64 - 1.0);
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [(-dx, from.0), (dx, max_x - from.0), (-dy, from.1), (dy, max_y - from.1)] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let r = q / p;
        if p < 0.0 {
            if r > t1 { return None; }
            t0 = t0.max(r);
        } else {
            if r < t0 { return None; }
            t1 = t1.min(r);
        }
    }
    let at = |t: f64| ((from.0 + t * dx).round() as i128, (from.1 + t * dy).round() as i128);
    Some((at(t0), at(t1)))
}

/// Tracé de Bresenham entre deux pixels de l'image.
pub(crate) fn line(pixels: &mut RgbaImage, (mut x, mut y): (i128, i128), (x1, y1): (i128, i128), color: Rgba<u8>) {
    let (dx, dy) = ((x1 - x).abs(), -(y1 - y).abs());
    let (sx, sy) = (if x < x1 { 1 } else { -1 }, if y < y1 { 1 } else { -1 });
    let mut err = dx + dy;
    loop {
        plot(pixels, x, y, color);
        if x == x1 && y == y1 {
            return;
        }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x += sx; }
        if e2 <= dx { err += dx; y += sy; }
    }
}

/// Demi-largeur de la ligne `dy` d'un disque de rayon `radius` (pixels à
/// une distance du centre au plus `radius`), `None` hors du disque.
pub(crate) fn half_width(radius: f64, dy: i128) -> Option<i128> {
    let rest = radius * radius - (dy * dy) as f64;
    (rest >= 0.0).then(|| rest.sqrt().floor() as i128)
}

/// Image tournée de `degrees` dans le sens horaire, agrandie pour tout
/// contenir, fond transparent, échantillonnage bilinéaire en alpha prémultiplié.
pub(crate) fn rotate_any(src: &RgbaImage, degrees: f64, size: (u32, u32)) -> RgbaImage {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (cx, cy) = (src.width() as f64 / 2.0, src.height() as f64 / 2.0);
    let (ncx, ncy) = (size.0 as f64 / 2.0, size.1 as f64 / 2.0);
    RgbaImage::from_fn(size.0, size.1, |x, y| {
        let (dx, dy) = (x as f64 + 0.5 - ncx, y as f64 + 0.5 - ncy);
        sample(src, dx * cos + dy * sin + cx - 0.5, -dx * sin + dy * cos + cy - 0.5)
    })
}

/// Dimensions de l'image tournée de `degrees`.
pub(crate) fn rotated_size(src: &RgbaImage, degrees: f64) -> (i64, i64) {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (w, h) = (src.width() as f64, src.height() as f64);
    let extent = |v: f64| (v - 1e-9).ceil().max(1.0) as i64;
    (extent(w * cos.abs() + h * sin.abs()), extent(w * sin.abs() + h * cos.abs()))
}

fn sample(src: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let mut acc = [0.0_f64; 4];
    for (ox, oy, weight) in [(0, 0, (1.0 - fx) * (1.0 - fy)), (1, 0, fx * (1.0 - fy)), (0, 1, (1.0 - fx) * fy), (1, 1, fx * fy)] {
        let (px, py) = (x0 as i64 + ox, y0 as i64 + oy);
        if weight == 0.0 || px < 0 || py < 0 || px >= src.width() as i64 || py >= src.height() as i64 {
            continue;
        }
        let p = src.get_pixel(px as u32, py as u32);
        let alpha = p[3] as f64 * weight;
        for i in 0..3 {
            acc[i] += p[i] as f64 * alpha;
        }
        acc[3] += alpha;
    }
    if acc[3] <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let channel = |v: f64| v.round().clamp(0.0, 255.0) as u8;
    Rgba([channel(acc[0] / acc[3]), channel(acc[1] / acc[3]), channel(acc[2] / acc[3]), channel(acc[3])])
}
