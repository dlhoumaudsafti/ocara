// src/codegen/desc.d/image.rs — signatures Cranelift pour builtin Image
// (crate runtime_image). `this` : la valeur Image elle-même.

use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

const fn image(name: &'static str, params: &'static [clt::Type], returns: Option<clt::Type>) -> BuiltinDesc {
    BuiltinDesc { name, params, returns, module: Some("Image") }
}

const I: clt::Type = clt::I64;

pub const IMAGE_BUILTINS: &[BuiltinDesc] = &[
    image("Image_create",       &[I, I],                Some(I)),
    image("Image_load",         &[I],                   Some(I)),
    image("Image_width",        &[I],                   Some(I)),
    image("Image_height",       &[I],                   Some(I)),
    image("Image_copy",         &[I],                   Some(I)),
    image("Image_save",         &[I, I],                None),
    image("Image_saveJpeg",     &[I, I, I],             None),
    image("Image_getPixel",     &[I, I, I],             Some(I)),
    image("Image_setPixel",     &[I, I, I, I, I, I, I], None),
    image("Image_setDrawColor", &[I, I, I, I, I],       None),
    image("Image_clear",        &[I],                   None),
    image("Image_drawPoint",    &[I, I, I],             None),
    image("Image_drawLine",     &[I, I, I, I, I],       None),
    image("Image_drawRect",     &[I, I, I, I, I],       None),
    image("Image_fillRect",     &[I, I, I, I, I],       None),
    image("Image_drawCircle",   &[I, I, I, I],          None),
    image("Image_fillCircle",   &[I, I, I, I],          None),
    image("Image_drawImage",    &[I, I, I, I],          None),
    image("Image_resize",       &[I, I, I],             None),
    image("Image_crop",         &[I, I, I, I, I],       None),
    image("Image_rotate",       &[I, clt::F64],         None),
    image("Image_flipH",        &[I],                   None),
    image("Image_flipV",        &[I],                   None),
];
