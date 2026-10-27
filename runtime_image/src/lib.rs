// runtime_image/src/lib.rs — ocara.Image : images en mémoire, hors fenêtre
// (création, chargement, dessin, transformations, enregistrement). Crate
// séparé d'ocara_runtime (voir Cargo.toml). Voir docs/builtins/Image.md.
//
// Une `Image` est une valeur native comptée (runtime/src/native.rs) :
// libérée avec sa dernière référence, jamais copiée au passage d'argument.
// Une exception est levée par `longjmp`, qui ne rend aucun verrou : elle
// part toujours après la libération du verrou de l'image (`with_canvas`).

mod codec;
mod draw;
mod ffi;
mod raster;
mod transform;

#[cfg(test)]
extern crate ocara_runtime;

use std::sync::{Mutex, MutexGuard};

use image::{Rgba, RgbaImage};

use ffi::{native_drop, native_new, native_ref, raise};

const ERR_INVALID: i64 = 100;
const ERR_READ: i64 = 101;
const ERR_DECODE: i64 = 102;
const ERR_WRITE: i64 = 103;
const ERR_FORMAT: i64 = 104;
const ERR_SIZE: i64 = 105;
const ERR_BOUNDS: i64 = 106;
const ERR_VALUE: i64 = 107;

/// Plus grande image acceptée (1 Gio de pixels RGBA).
const MAX_PIXELS: u64 = 1 << 28;

/// Pixels d'une image et couleur courante des primitives de dessin.
struct Canvas {
    pixels: RgbaImage,
    color:  Rgba<u8>,
}

struct OcaraImage(Mutex<Canvas>);

struct Failure {
    code:    i64,
    message: String,
}

impl Failure {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Failure { code, message: message.into() }
    }
}

fn fail(failure: Failure) -> ! {
    raise("ImageException", "Image", failure.message, failure.code)
}

extern "C" fn drop_image(data: i64) {
    unsafe { native_drop::<OcaraImage>(data) }
}

fn new_image(pixels: RgbaImage, color: Rgba<u8>) -> i64 {
    native_new(OcaraImage(Mutex::new(Canvas { pixels, color })), drop_image)
}

fn image_of(this: i64) -> &'static OcaraImage {
    native_ref::<OcaraImage>(this, drop_image).unwrap_or_else(|| fail(Failure::new(ERR_INVALID, "valeur qui n'est pas une Image")))
}

fn lock(image: &OcaraImage) -> MutexGuard<'_, Canvas> {
    image.0.lock().unwrap_or_else(|e| e.into_inner())
}

/// Exécute `f` sous le verrou de l'image, puis lève son échec éventuel.
fn with_canvas<R>(this: i64, f: impl FnOnce(&mut Canvas) -> Result<R, Failure>) -> R {
    let result = f(&mut lock(image_of(this)));
    result.unwrap_or_else(|e| fail(e))
}

/// Dimensions d'une image à créer, en pixels.
fn dimensions(width: i64, height: i64) -> Result<(u32, u32), Failure> {
    let valid = width > 0 && height > 0 && (width as u64).saturating_mul(height as u64) <= MAX_PIXELS;
    if !valid {
        return Err(Failure::new(ERR_SIZE, format!("dimensions invalides : {width}x{height}")));
    }
    Ok((width as u32, height as u32))
}

fn rgba(r: i64, g: i64, b: i64, a: i64) -> Result<Rgba<u8>, Failure> {
    let channel = |v: i64| u8::try_from(v).map_err(|_| Failure::new(ERR_VALUE, format!("composante de couleur hors de 0..255 : {v}")));
    Ok(Rgba([channel(r)?, channel(g)?, channel(b)?, channel(a)?]))
}

/// Coordonnées d'un pixel existant de `pixels`.
fn inside(pixels: &RgbaImage, x: i64, y: i64) -> Result<(u32, u32), Failure> {
    if x < 0 || y < 0 || x >= pixels.width() as i64 || y >= pixels.height() as i64 {
        return Err(Failure::new(ERR_BOUNDS, format!("pixel ({x}, {y}) hors de l'image {}x{}", pixels.width(), pixels.height())));
    }
    Ok((x as u32, y as u32))
}

#[cfg(test)]
mod tests;
