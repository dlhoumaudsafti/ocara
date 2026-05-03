use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module HTTPRequest
pub const HTTPREQUEST_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "HTTPRequest_new",        params: &[clt::I64],                   returns: Some(clt::I64), module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_setMethod",  params: &[clt::I64, clt::I64],         returns: None,           module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_setHeader",  params: &[clt::I64, clt::I64, clt::I64], returns: None,         module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_setBody",    params: &[clt::I64, clt::I64],         returns: None,           module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_setTimeout", params: &[clt::I64, clt::I64],         returns: None,           module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_send",       params: &[clt::I64],                   returns: Some(clt::I64), module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_status",     params: &[clt::I64],                   returns: Some(clt::I64), module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_body",       params: &[clt::I64],                   returns: Some(clt::I64), module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_header",     params: &[clt::I64, clt::I64],         returns: Some(clt::I64), module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_headers",    params: &[clt::I64],                   returns: Some(clt::I64), module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_ok",         params: &[clt::I64],                   returns: Some(clt::I64), module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_isError",    params: &[clt::I64],                   returns: Some(clt::I64), module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_error",      params: &[clt::I64],                   returns: Some(clt::I64), module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_get",        params: &[clt::I64],                   returns: Some(clt::I64), module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_post",       params: &[clt::I64, clt::I64],         returns: Some(clt::I64), module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_put",        params: &[clt::I64, clt::I64],         returns: Some(clt::I64), module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_delete",     params: &[clt::I64],                   returns: Some(clt::I64), module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_patch",      params: &[clt::I64, clt::I64],         returns: Some(clt::I64), module: Some("HTTPRequest") },
];
