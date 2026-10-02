use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module HTTPServer (le serveur lui-même) + HTTPServerRequest
/// (l'objet requête passé aux handlers — voir
/// docs/roadmap.d/stdlib-httpserver-request-object.md, désormais clos).
pub const HTTPSERVER_BUILTINS: &[BuiltinDesc] = &[
    // ── HTTPServer (instance) ────────────────────────────────────────────────
    BuiltinDesc { name: "HTTPServer_init",          params: &[clt::I64],                            returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_port",       params: &[clt::I64, clt::I64],                  returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_host",       params: &[clt::I64, clt::I64],                  returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_workers",    params: &[clt::I64, clt::I64],                  returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_rootPath",   params: &[clt::I64, clt::I64],                  returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_route",         params: &[clt::I64, clt::I64, clt::I64, clt::I64], returns: None,        module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_routeError",    params: &[clt::I64, clt::I64, clt::I64],        returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_run",           params: &[clt::I64],                            returns: None,           module: Some("HTTPServer") },
    // ── HTTPServerRequest (méthodes appelées depuis un handler) ──────────────
    BuiltinDesc { name: "HTTPServerRequest_path",       params: &[clt::I64],                            returns: Some(clt::I64), module: Some("HTTPServerRequest") },
    BuiltinDesc { name: "HTTPServerRequest_method",     params: &[clt::I64],                            returns: Some(clt::I64), module: Some("HTTPServerRequest") },
    BuiltinDesc { name: "HTTPServerRequest_body",       params: &[clt::I64],                            returns: Some(clt::I64), module: Some("HTTPServerRequest") },
    BuiltinDesc { name: "HTTPServerRequest_header",     params: &[clt::I64, clt::I64],                  returns: Some(clt::I64), module: Some("HTTPServerRequest") },
    BuiltinDesc { name: "HTTPServerRequest_headers",    params: &[clt::I64],                            returns: Some(clt::I64), module: Some("HTTPServerRequest") },
    BuiltinDesc { name: "HTTPServerRequest_query",      params: &[clt::I64, clt::I64],                  returns: Some(clt::I64), module: Some("HTTPServerRequest") },
    // param(key) / param(key, method) — surcharge par arité, voir
    // src/lower/expr.d/lower.rs (suffixe _N = nombre d'arguments EXPLICITES,
    // récepteur exclu ; la forme complète, method inclus, reste SANS suffixe
    // — même convention que SQLite_execute_1/_2/SQLite_execute).
    BuiltinDesc { name: "HTTPServerRequest_param_1",    params: &[clt::I64, clt::I64],                  returns: Some(clt::I64), module: Some("HTTPServerRequest") },
    BuiltinDesc { name: "HTTPServerRequest_param",      params: &[clt::I64, clt::I64, clt::I64],        returns: Some(clt::I64), module: Some("HTTPServerRequest") },
    BuiltinDesc { name: "HTTPServerRequest_params",     params: &[clt::I64],                            returns: Some(clt::I64), module: Some("HTTPServerRequest") },
    BuiltinDesc { name: "HTTPServerRequest_respond",       params: &[clt::I64, clt::I64, clt::I64],        returns: None,           module: Some("HTTPServerRequest") },
    BuiltinDesc { name: "HTTPServerRequest_respondHeader", params: &[clt::I64, clt::I64, clt::I64],        returns: None,           module: Some("HTTPServerRequest") },
    BuiltinDesc { name: "HTTPServerRequest_cookie",     params: &[clt::I64, clt::I64],                  returns: Some(clt::I64), module: Some("HTTPServerRequest") },
    BuiltinDesc { name: "HTTPServerRequest_session",    params: &[clt::I64],                            returns: Some(clt::I64), module: Some("HTTPServerRequest") },
    // ── HTTPServerSession (runtime/src/httpsession.rs) — `set`/`setGlobal`
    // reçoivent en dernier argument caché la forme de la valeur, voir
    // `static_leaf_shape` (src/lower/expr.d/helpers.rs).
    BuiltinDesc { name: "HTTPServerSession_id",           params: &[clt::I64],                               returns: Some(clt::I64), module: Some("HTTPServerSession") },
    BuiltinDesc { name: "HTTPServerSession_set",          params: &[clt::I64, clt::I64, clt::I64, clt::I64], returns: None,           module: Some("HTTPServerSession") },
    BuiltinDesc { name: "HTTPServerSession_get",          params: &[clt::I64, clt::I64],                     returns: Some(clt::I64), module: Some("HTTPServerSession") },
    BuiltinDesc { name: "HTTPServerSession_has",          params: &[clt::I64, clt::I64],                     returns: Some(clt::I64), module: Some("HTTPServerSession") },
    BuiltinDesc { name: "HTTPServerSession_remove",       params: &[clt::I64, clt::I64],                     returns: None,           module: Some("HTTPServerSession") },
    BuiltinDesc { name: "HTTPServerSession_destroy",      params: &[clt::I64],                               returns: None,           module: Some("HTTPServerSession") },
    BuiltinDesc { name: "HTTPServerSession_setGlobal",    params: &[clt::I64, clt::I64, clt::I64],           returns: None,           module: Some("HTTPServerSession") },
    BuiltinDesc { name: "HTTPServerSession_getGlobal",    params: &[clt::I64],                               returns: Some(clt::I64), module: Some("HTTPServerSession") },
    BuiltinDesc { name: "HTTPServerSession_hasGlobal",    params: &[clt::I64],                               returns: Some(clt::I64), module: Some("HTTPServerSession") },
    BuiltinDesc { name: "HTTPServerSession_removeGlobal", params: &[clt::I64],                               returns: None,           module: Some("HTTPServerSession") },
];
