// Rastérisation et validation des arguments (sans exception : une exception
// Ocara part par `longjmp`, hors d'un programme compilé).

use image::{Rgba, RgbaImage};

use crate::raster::*;
use crate::*;

const RED: Rgba<u8> = Rgba([255, 0, 0, 255]);

fn count(pixels: &RgbaImage, color: Rgba<u8>) -> usize {
    pixels.pixels().filter(|p| **p == color).count()
}

#[test]
fn blend_opaque_replaces_and_transparent_keeps() {
    let mut dst = Rgba([10, 20, 30, 255]);
    blend(&mut dst, Rgba([1, 2, 3, 0]));
    assert_eq!(dst, Rgba([10, 20, 30, 255]));
    blend(&mut dst, RED);
    assert_eq!(dst, RED);
}

#[test]
fn blend_half_alpha_over_opaque() {
    let mut dst = Rgba([0, 0, 255, 255]);
    blend(&mut dst, Rgba([255, 0, 0, 128]));
    assert_eq!(dst, Rgba([128, 0, 127, 255]));
}

#[test]
fn blend_over_transparent_keeps_source() {
    let mut dst = Rgba([0, 0, 0, 0]);
    blend(&mut dst, Rgba([200, 100, 50, 77]));
    assert_eq!(dst, Rgba([200, 100, 50, 77]));
}

#[test]
fn line_is_clipped_to_the_image() {
    let mut pixels = RgbaImage::new(10, 10);
    let (from, to) = clip_line(&pixels, (-1e12, 5.0), (1e12, 5.0)).unwrap();
    assert_eq!((from, to), ((0, 5), (9, 5)));
    line(&mut pixels, from, to, RED);
    assert_eq!(count(&pixels, RED), 10);
    assert!(clip_line(&pixels, (-5.0, -5.0), (-1.0, 20.0)).is_none());
}

#[test]
fn diagonal_line_has_one_pixel_per_column() {
    let mut pixels = RgbaImage::new(5, 5);
    let (from, to) = clip_line(&pixels, (0.0, 0.0), (4.0, 4.0)).unwrap();
    line(&mut pixels, from, to, RED);
    assert_eq!(count(&pixels, RED), 5);
}

#[test]
fn span_ignores_what_is_outside() {
    let mut pixels = RgbaImage::new(4, 4);
    span(&mut pixels, 1, -100, 100, RED);
    span(&mut pixels, -1, 0, 3, RED);
    span(&mut pixels, i128::MAX, 0, 3, RED);
    assert_eq!(count(&pixels, RED), 4);
    assert_eq!(rows(&pixels, -10, 1), 0..=1);
}

#[test]
fn disk_rows_follow_the_radius() {
    assert_eq!(half_width(2.5, 0), Some(2));
    assert_eq!(half_width(2.5, 2), Some(1));
    assert_eq!(half_width(2.5, 3), None);
}

#[test]
fn quarter_turn_size_swaps_dimensions() {
    let pixels = RgbaImage::new(30, 10);
    assert_eq!(rotated_size(&pixels, 90.0), (10, 30));
    assert_eq!(rotated_size(&pixels, 45.0), (29, 29));
}

#[test]
fn free_rotation_keeps_an_opaque_center() {
    let pixels = RgbaImage::from_pixel(20, 20, RED);
    let (w, h) = rotated_size(&pixels, 45.0);
    let rotated = rotate_any(&pixels, 45.0, (w as u32, h as u32));
    assert_eq!(*rotated.get_pixel(w as u32 / 2, h as u32 / 2), RED);
    assert_eq!(rotated.get_pixel(0, 0)[3], 0);
}

#[test]
fn arguments_are_validated() {
    assert!(dimensions(0, 5).is_err());
    assert!(dimensions(-1, 5).is_err());
    assert!(dimensions(1 << 15, 1 << 15).is_err());
    assert_eq!(dimensions(3, 2).ok(), Some((3, 2)));
    assert!(rgba(0, 0, 0, 256).is_err());
    assert!(rgba(-1, 0, 0, 0).is_err());
    let pixels = RgbaImage::new(3, 3);
    assert!(inside(&pixels, 3, 0).is_err());
    assert_eq!(inside(&pixels, 2, 2).ok(), Some((2, 2)));
}
