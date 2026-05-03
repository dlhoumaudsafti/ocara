use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module HTMLComponent
pub const HTMLCOMPONENT_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "HTMLComponent_init",     params: &[clt::I64, clt::I64], returns: None, module: Some("HTMLComponent") },
    BuiltinDesc { name: "HTMLComponent_register", params: &[clt::I64, clt::I64], returns: None, module: Some("HTMLComponent") },
];

/// Builtins du module HTML
pub const HTML_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "HTML_render",       params: &[clt::I64],               returns: Some(clt::I64), module: Some("HTML") },
    BuiltinDesc { name: "HTML_renderCached", params: &[clt::I64, clt::I64],     returns: Some(clt::I64), module: Some("HTML") },
    BuiltinDesc { name: "HTML_cacheDelete",  params: &[clt::I64],               returns: None,           module: Some("HTML") },
    BuiltinDesc { name: "HTML_cacheClear",   params: &[],                       returns: None,           module: Some("HTML") },
    BuiltinDesc { name: "HTML_escape",       params: &[clt::I64],               returns: Some(clt::I64), module: Some("HTML") },
];
