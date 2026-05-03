use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module HTTPServer
pub const HTTPSERVER_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "HTTPServer_init",          params: &[clt::I64],                            returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_port",       params: &[clt::I64, clt::I64],                  returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_host",       params: &[clt::I64, clt::I64],                  returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_workers",    params: &[clt::I64, clt::I64],                  returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_rootPath",   params: &[clt::I64, clt::I64],                  returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_route",         params: &[clt::I64, clt::I64, clt::I64, clt::I64], returns: None,        module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_routeError",    params: &[clt::I64, clt::I64, clt::I64],        returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_run",           params: &[clt::I64],                            returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_path",       params: &[clt::I64],                            returns: Some(clt::I64), module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_method",     params: &[clt::I64],                            returns: Some(clt::I64), module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_body",       params: &[clt::I64],                            returns: Some(clt::I64), module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_header",     params: &[clt::I64, clt::I64],                  returns: Some(clt::I64), module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_query",      params: &[clt::I64, clt::I64],                  returns: Some(clt::I64), module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_respond",       params: &[clt::I64, clt::I64, clt::I64],        returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_respondHeader", params: &[clt::I64, clt::I64, clt::I64],        returns: None,           module: Some("HTTPServer") },
];
