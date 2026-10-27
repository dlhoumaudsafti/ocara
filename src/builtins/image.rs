// ─────────────────────────────────────────────────────────────────────────────
// ocara.Image — images en mémoire, hors fenêtre (crate runtime_image)
//
// Méthodes statiques :
//   Image::create(width, height) → Image          // transparente
//   Image::load(path) → Image                     // format reconnu au contenu
//
// Méthodes d'instance :
//   img.width() / img.height() → int / img.copy() → Image
//   img.save(path) / img.saveJpeg(path, quality)  // format choisi par l'extension
//   img.getPixel(x, y) → array<int> / img.setPixel(x, y, r, g, b, a)
//   img.setDrawColor(r, g, b, a) / img.clear()
//   img.drawPoint / drawLine / drawRect / fillRect / drawCircle / fillCircle
//   img.drawImage(src, x, y)                      // composition alpha
//   img.resize(width, height) / crop(x, y, w, h) / rotate(degrees) / flipH() / flipV()
//
// Convention runtime : Image_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

fn static_m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: true,
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
        required_params_count: len,
        message_emit_in_loop: false,
    }
}

fn inst_m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: false,
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
        required_params_count: len,
        message_emit_in_loop: false,
    }
}

fn image_ty() -> Type { Type::Named("Image".to_string()) }

pub fn class() -> ClassInfo {
    let mut methods = HashMap::new();

    // Création et chargement
    methods.insert("create".to_string(), static_m(vec![("width", Type::Int), ("height", Type::Int)], image_ty()));
    methods.insert("load".to_string(), static_m(vec![("path", Type::String)], image_ty()));

    // Dimensions, copie, enregistrement
    methods.insert("width".to_string(), inst_m(vec![], Type::Int));
    methods.insert("height".to_string(), inst_m(vec![], Type::Int));
    methods.insert("copy".to_string(), inst_m(vec![], image_ty()));
    methods.insert("save".to_string(), inst_m(vec![("path", Type::String)], Type::Void));
    methods.insert("saveJpeg".to_string(), inst_m(vec![("path", Type::String), ("quality", Type::Int)], Type::Void));

    // Pixels et dessin
    methods.insert("getPixel".to_string(), inst_m(vec![("x", Type::Int), ("y", Type::Int)], Type::Array(Box::new(Type::Int))));
    methods.insert("setPixel".to_string(), inst_m(
        vec![("x", Type::Int), ("y", Type::Int), ("r", Type::Int), ("g", Type::Int), ("b", Type::Int), ("a", Type::Int)],
        Type::Void,
    ));
    methods.insert("setDrawColor".to_string(), inst_m(
        vec![("r", Type::Int), ("g", Type::Int), ("b", Type::Int), ("a", Type::Int)],
        Type::Void,
    ));
    methods.insert("clear".to_string(), inst_m(vec![], Type::Void));
    methods.insert("drawPoint".to_string(), inst_m(vec![("x", Type::Int), ("y", Type::Int)], Type::Void));
    methods.insert("drawLine".to_string(), inst_m(
        vec![("x1", Type::Int), ("y1", Type::Int), ("x2", Type::Int), ("y2", Type::Int)],
        Type::Void,
    ));
    methods.insert("drawRect".to_string(), inst_m(
        vec![("x", Type::Int), ("y", Type::Int), ("w", Type::Int), ("h", Type::Int)],
        Type::Void,
    ));
    methods.insert("fillRect".to_string(), inst_m(
        vec![("x", Type::Int), ("y", Type::Int), ("w", Type::Int), ("h", Type::Int)],
        Type::Void,
    ));
    methods.insert("drawCircle".to_string(), inst_m(vec![("cx", Type::Int), ("cy", Type::Int), ("radius", Type::Int)], Type::Void));
    methods.insert("fillCircle".to_string(), inst_m(vec![("cx", Type::Int), ("cy", Type::Int), ("radius", Type::Int)], Type::Void));
    methods.insert("drawImage".to_string(), inst_m(vec![("src", image_ty()), ("x", Type::Int), ("y", Type::Int)], Type::Void));

    // Transformations sur place
    methods.insert("resize".to_string(), inst_m(vec![("width", Type::Int), ("height", Type::Int)], Type::Void));
    methods.insert("crop".to_string(), inst_m(
        vec![("x", Type::Int), ("y", Type::Int), ("w", Type::Int), ("h", Type::Int)],
        Type::Void,
    ));
    methods.insert("rotate".to_string(), inst_m(vec![("degrees", Type::Float)], Type::Void));
    methods.insert("flipH".to_string(), inst_m(vec![], Type::Void));
    methods.insert("flipV".to_string(), inst_m(vec![], Type::Void));

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}
