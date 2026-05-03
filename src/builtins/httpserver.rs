// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTTPServer — classe builtin d'instance
//
// Méthodes d'instance (is_static: false) :
//   s.port(port:int)                         → void
//   s.host(host:string)                      → void
//   s.workers(n:int)                         → void
//   s.rootPath(path:string)                 → void
//   s.route(path:string, method:string, f:Function) → void
//   s.run()                                      → void  (bloquant)
//
// Méthodes statiques (is_static: true) — appelées depuis un handler :
//   HTTPServer::path(req:int)                → string
//   HTTPServer::method(req:int)              → string
//   HTTPServer::body(req:int)                → string
//   HTTPServer::header(req:int, name:string) → string
//   HTTPServer::query(req:int, key:string)   → string
//   HTTPServer::respond(req:int, status:int, body:string) → void
//   HTTPServer::respondHeader(req:int, name:string, value:string) → void
//
// Convention runtime : HTTPServer_<method>
// Usage :
//   const server:HTTPServer = use HTTPServer()
//   server.port(8080)
//   server.route("/", "GET", nameless(req:int): int {
//       HTTPServer::respond(req, 200, "Hello")
//       return 0
//   })
//   server.run()
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::Type;
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

    // s.port(port:int) → void
    methods.insert("port".into(), instance(
        vec![("port", Type::Int)],
        Type::Void,
    ));

    // s.host(host:string) → void
    methods.insert("host".into(), instance(
        vec![("host", Type::String)],
        Type::Void,
    ));

    // s.workers(n:int) → void
    methods.insert("workers".into(), instance(
        vec![("n", Type::Int)],
        Type::Void,
    ));

    // s.rootPath(path:string) → void
    methods.insert("rootPath".into(), instance(
        vec![("path", Type::String)],
        Type::Void,
    ));

    // s.route(path:string, method:string, f:Function<void>) → void
    methods.insert("route".into(), instance(
        vec![
            ("path",   Type::String),
            ("method", Type::String),
            ("f",      Type::Function { ret_ty: Box::new(Type::Void), param_tys: vec![] }),
        ],
        Type::Void,
    ));

    // s.routeError(code:int, f:Function<void>) → void
    methods.insert("routeError".into(), instance(
        vec![
            ("code", Type::Int),
            ("f",    Type::Function { ret_ty: Box::new(Type::Void), param_tys: vec![] }),
        ],
        Type::Void,
    ));

    // s.run() → void  (bloquant)
    methods.insert("run".into(), instance(
        vec![],
        Type::Void,
    ));

    // ── Méthodes statiques — lecture de la requête ───────────────────────────

    // HTTPServer::path(req:int) → string
    methods.insert("path".into(), static_m(
        vec![("req", Type::Int)],
        Type::String,
    ));

    // HTTPServer::method(req:int) → string
    methods.insert("method".into(), static_m(
        vec![("req", Type::Int)],
        Type::String,
    ));

    // HTTPServer::body(req:int) → string
    methods.insert("body".into(), static_m(
        vec![("req", Type::Int)],
        Type::String,
    ));

    // HTTPServer::header(req:int, name:string) → string
    methods.insert("header".into(), static_m(
        vec![("req", Type::Int), ("name", Type::String)],
        Type::String,
    ));

    // HTTPServer::query(req:int, key:string) → string
    methods.insert("query".into(), static_m(
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

    // HTTPServer::respondHeader(req:int, name:string, value:string) → void
    methods.insert("respondHeader".into(), static_m(
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
