/// Builtins déclarés côté Ocara runtime.
///
/// Ces fonctions sont résolues à la liaison (link) depuis la bibliothèque
/// runtime C minimale `ocara_runtime.c` (ou implémentées inline via Cranelift).
///
/// Chaque entry décrit la signature attendue par Cranelift.
use cranelift_codegen::ir::{AbiParam, Signature};
use cranelift_codegen::isa::CallConv;

use super::desc::{
    LOWLEVEL_BUILTINS,
    IO_BUILTINS,
    SYSTEM_BUILTINS,
    THREAD_BUILTINS,
    MUTEX_BUILTINS,
    MATH_BUILTINS,
    STRING_BUILTINS,
    ARRAY_BUILTINS,
    MAP_BUILTINS,
    REGEX_BUILTINS,
    CONVERT_BUILTINS,
    DATETIME_BUILTINS,
    DATE_BUILTINS,
    TIME_BUILTINS,
    FILE_BUILTINS,
    DIRECTORY_BUILTINS,
    JSON_BUILTINS,
    HTTPREQUEST_BUILTINS,
    HTTPSERVER_BUILTINS,
    HTML_BUILTINS,
    HTMLCOMPONENT_BUILTINS,
    UNITTEST_BUILTINS,
};

#[derive(Clone, Copy)]
pub struct BuiltinDesc {
    pub name:    &'static str,
    pub params:  &'static [cranelift_codegen::ir::Type],
    pub returns: Option<cranelift_codegen::ir::Type>,
    /// Module ocara requis pour utiliser ce builtin (ex: "Array", "IO").
    /// `None` = interne (toujours disponible, pas d'import requis).
    pub module:  Option<&'static str>,
}

// Fonction pour obtenir tous les builtins combinés
use std::sync::OnceLock;
static BUILTINS_COMBINED: OnceLock<Vec<BuiltinDesc>> = OnceLock::new();

pub fn builtins() -> &'static [BuiltinDesc] {
    BUILTINS_COMBINED.get_or_init(|| {
        let mut all = Vec::new();
        // Builtins internes et méthodes d'instance par ordre de niveau d'importance
        all.extend_from_slice(LOWLEVEL_BUILTINS);
        all.extend_from_slice(IO_BUILTINS);
        all.extend_from_slice(SYSTEM_BUILTINS);
        all.extend_from_slice(THREAD_BUILTINS);
        all.extend_from_slice(MUTEX_BUILTINS);
        all.extend_from_slice(MATH_BUILTINS);
        all.extend_from_slice(STRING_BUILTINS);
        all.extend_from_slice(ARRAY_BUILTINS);
        all.extend_from_slice(MAP_BUILTINS);
        all.extend_from_slice(REGEX_BUILTINS);
        all.extend_from_slice(CONVERT_BUILTINS);
        all.extend_from_slice(DATETIME_BUILTINS);
        all.extend_from_slice(DATE_BUILTINS);
        all.extend_from_slice(TIME_BUILTINS);
        all.extend_from_slice(FILE_BUILTINS);
        all.extend_from_slice(JSON_BUILTINS);
        all.extend_from_slice(DIRECTORY_BUILTINS);
        all.extend_from_slice(HTTPREQUEST_BUILTINS);
        all.extend_from_slice(HTTPSERVER_BUILTINS);
        all.extend_from_slice(HTML_BUILTINS);
        all.extend_from_slice(HTMLCOMPONENT_BUILTINS);
        all.extend_from_slice(UNITTEST_BUILTINS);
        all
    })
}

// Pour rétrocompatibilité (deprecated, utiliser builtins() à la place)
pub const BUILTINS: &[BuiltinDesc] = &[];

pub fn builtin_sig(desc: &BuiltinDesc, call_conv: CallConv) -> Signature {
    let mut sig = Signature::new(call_conv);
    for &ty in desc.params {
        sig.params.push(AbiParam::new(ty));
    }
    if let Some(ret) = desc.returns {
        sig.returns.push(AbiParam::new(ret));
    }
    sig
}
