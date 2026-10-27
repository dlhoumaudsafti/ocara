// Création, chargement, enregistrement et copie d'une image.

use std::fs::File;
use std::io::{BufWriter, Write};

use image::buffer::ConvertBuffer;
use image::codecs::jpeg::JpegEncoder;
use image::{ImageError, ImageFormat, ImageReader, Rgba, RgbaImage, RgbImage};

use crate::ffi::str_arg;
use crate::*;

const DEFAULT_JPEG_QUALITY: u8 = 90;
const OPAQUE_BLACK: Rgba<u8> = Rgba([0, 0, 0, 255]);

/// Image::create(width, height) → Image, entièrement transparente.
#[unsafe(no_mangle)]
pub extern "C" fn Image_create(width: i64, height: i64) -> i64 {
    let (w, h) = dimensions(width, height).unwrap_or_else(|e| fail(e));
    new_image(RgbaImage::new(w, h), OPAQUE_BLACK)
}

/// Image::load(path) → Image, format reconnu au contenu du fichier.
#[unsafe(no_mangle)]
pub extern "C" fn Image_load(path_ptr: i64) -> i64 {
    let path = str_arg(path_ptr);
    let pixels = decode(path).unwrap_or_else(|e| fail(e));
    new_image(pixels, OPAQUE_BLACK)
}

fn decode(path: &str) -> Result<RgbaImage, Failure> {
    let unreadable = |e: std::io::Error| Failure::new(ERR_READ, format!("lecture de '{path}' impossible : {e}"));
    let reader = ImageReader::open(path).map_err(unreadable)?.with_guessed_format().map_err(unreadable)?;
    if reader.format().is_none() {
        return Err(Failure::new(ERR_DECODE, format!("format d'image non reconnu : '{path}'")));
    }
    let image = reader.decode().map_err(|e| Failure::new(ERR_DECODE, format!("décodage de '{path}' impossible : {e}")))?;
    Ok(image.to_rgba8())
}

/// img.save(path), format choisi par l'extension.
#[unsafe(no_mangle)]
pub extern "C" fn Image_save(this: i64, path_ptr: i64) {
    save(this, path_ptr, DEFAULT_JPEG_QUALITY);
}

/// img.saveJpeg(path, quality), qualité de 1 à 100.
#[unsafe(no_mangle)]
pub extern "C" fn Image_saveJpeg(this: i64, path_ptr: i64, quality: i64) {
    let quality = u8::try_from(quality).ok().filter(|q| (1..=100).contains(q))
        .unwrap_or_else(|| fail(Failure::new(ERR_VALUE, format!("qualité JPEG hors de 1..100 : {quality}"))));
    save(this, path_ptr, quality);
}

fn save(this: i64, path_ptr: i64, quality: u8) {
    let path = str_arg(path_ptr);
    with_canvas(this, |c| encode(&c.pixels, path, quality));
}

fn encode(pixels: &RgbaImage, path: &str, quality: u8) -> Result<(), Failure> {
    let format = ImageFormat::from_path(path).ok().filter(|f| f.writing_enabled()).ok_or_else(|| {
        Failure::new(ERR_FORMAT, format!("format d'enregistrement non pris en charge : '{path}' (png, jpg, webp, gif, bmp)"))
    })?;
    let file = File::create(path).map_err(|e| Failure::new(ERR_WRITE, format!("écriture de '{path}' impossible : {e}")))?;
    let mut out = BufWriter::new(file);
    let written = match format {
        ImageFormat::Jpeg => {
            let rgb: RgbImage = pixels.convert();
            JpegEncoder::new_with_quality(&mut out, quality).encode_image(&rgb)
        }
        _ => pixels.write_to(&mut out, format),
    };
    written
        .and_then(|_| out.flush().map_err(ImageError::IoError))
        .map_err(|e| Failure::new(ERR_WRITE, format!("écriture de '{path}' impossible : {e}")))
}

/// img.copy() → Image, pixels et couleur de dessin compris.
#[unsafe(no_mangle)]
pub extern "C" fn Image_copy(this: i64) -> i64 {
    let (pixels, color) = with_canvas(this, |c| Ok((c.pixels.clone(), c.color)));
    new_image(pixels, color)
}

#[unsafe(no_mangle)]
pub extern "C" fn Image_width(this: i64) -> i64 {
    with_canvas(this, |c| Ok(c.pixels.width() as i64))
}

#[unsafe(no_mangle)]
pub extern "C" fn Image_height(this: i64) -> i64 {
    with_canvas(this, |c| Ok(c.pixels.height() as i64))
}
