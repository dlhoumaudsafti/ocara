// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTTPServer — classe builtin d'instance
//
// Méthodes d'instance (is_static: false) :
//   s.set_port(port:int)                         → void
//   s.set_host(host:string)                      → void
//   s.set_workers(n:int)                         → void
//   s.set_root_path(path:string)                 → void
//   s.route(path:string, method:string, f:Function) → void
//   s.run()                                      → void  (bloquant)
//
// Méthodes statiques (is_static: true) — appelées depuis un handler :
//   HTTPServer::req_path(req:int)                → string
//   HTTPServer::req_method(req:int)              → string
//   HTTPServer::req_body(req:int)                → string
//   HTTPServer::req_header(req:int, name:string) → string
//   HTTPServer::req_query(req:int, key:string)   → string
//   HTTPServer::respond(req:int, status:int, body:string) → void
//   HTTPServer::set_resp_header(req:int, name:string, value:string) → void
//
// Convention runtime : HTTPServer_<method>
// Usage :
//   const server:HTTPServer = use HTTPServer()
//   server.set_port(8080)
//   server.route("/", "GET", nameless(req:int): int {
//       HTTPServer::respond(req, 200, "Hello")
//       return 0
//   })
//   server.run()
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

fn instance(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: false,
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
        required_params_count: len,
    }
}

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
    }
}

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // ── Méthodes d'instance ───────────────────────────────────────────────────

    // s.set_port(port:int) → void
    methods.insert("set_port".into(), instance(
        vec![("port", Type::Int)],
        Type::Void,
    ));

    // s.set_host(host:string) → void
    methods.insert("set_host".into(), instance(
        vec![("host", Type::String)],
        Type::Void,
    ));

    // s.set_workers(n:int) → void
    methods.insert("set_workers".into(), instance(
        vec![("n", Type::Int)],
        Type::Void,
    ));

    // s.set_root_path(path:string) → void
    methods.insert("set_root_path".into(), instance(
        vec![("path", Type::String)],
        Type::Void,
    ));

    // s.route(path:string, method:string, f:Function<void>) → void
    methods.insert("route".into(), instance(
        vec![
            ("path",   Type::String),
            ("method", Type::String),
            ("f",      Type::Function(Box::new(Type::Void))),
        ],
        Type::Void,
    ));

    // s.route_error(code:int, f:Function<void>) → void
    methods.insert("route_error".into(), instance(
        vec![
            ("code", Type::Int),
            ("f",    Type::Function(Box::new(Type::Void))),
        ],
        Type::Void,
    ));

    // s.run() → void  (bloquant)
    methods.insert("run".into(), instance(
        vec![],
        Type::Void,
    ));

    // ── Méthodes statiques — lecture de la requête ───────────────────────────

    // HTTPServer::req_path(req:int) → string
    methods.insert("req_path".into(), static_m(
        vec![("req", Type::Int)],
        Type::String,
    ));

    // HTTPServer::req_method(req:int) → string
    methods.insert("req_method".into(), static_m(
        vec![("req", Type::Int)],
        Type::String,
    ));

    // HTTPServer::req_body(req:int) → string
    methods.insert("req_body".into(), static_m(
        vec![("req", Type::Int)],
        Type::String,
    ));

    // HTTPServer::req_header(req:int, name:string) → string
    methods.insert("req_header".into(), static_m(
        vec![("req", Type::Int), ("name", Type::String)],
        Type::String,
    ));

    // HTTPServer::req_query(req:int, key:string) → string
    methods.insert("req_query".into(), static_m(
        vec![("req", Type::Int), ("key", Type::String)],
        Type::String,
    ));

    // ── Méthodes statiques — construction de la réponse ──────────────────────

    // HTTPServer::respond(req:int, status:int, body:string) → void
    methods.insert("respond".into(), static_m(
        vec![
            ("req",    Type::Int),
            ("status", Type::Int),
            ("body",   Type::String),
        ],
        Type::Void,
    ));

    // HTTPServer::set_resp_header(req:int, name:string, value:string) → void
    methods.insert("set_resp_header".into(), static_m(
        vec![
            ("req",   Type::Int),
            ("name",  Type::String),
            ("value", Type::String),
        ],
        Type::Void,
    ));

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}
