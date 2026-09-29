// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTTPServer — classe builtin d'instance
// ocara.HTTPServerRequest — objet requête passé aux handlers (voir plus bas)
//
// Méthodes d'instance HTTPServer (is_static: false) :
//   s.port(port:int)                         → void
//   s.host(host:string)                      → void
//   s.workers(n:int)                         → void
//   s.rootPath(path:string)                 → void
//   s.route(path:string, method:string, f:Function) → void
//   s.run()                                      → void  (bloquant)
//
// Convention runtime : HTTPServer_<method>
// Usage :
//   const server:HTTPServer = use HTTPServer()
//   server.port(8080)
//   server.route("/", "GET", nameless(req:HTTPServerRequest): int {
//       req.respond(200, "Hello")
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
        message_emit_in_loop: false,
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
        message_emit_in_loop: false,
    }
}

/// Comme `static_m`, mais avec un nombre de paramètres REQUIS inférieur au
/// nombre total (paramètre(s) optionnel(s) en fin de liste) — même patron que
/// `sqlite::instance_opt` pour `queryOne(query, placeholder = null)`.
fn static_m_opt(params: Vec<(&str, Type)>, ret_ty: Type, required: usize) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: true,
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
        required_params_count: required,
        message_emit_in_loop: false,
    }
}

fn req_ty() -> Type { Type::Named("HTTPServerRequest".to_string()) }

/// `method:string|null` — paramètre optionnel de `param(key, method = null)`.
fn method_ty() -> Type {
    Type::Union(vec![Type::String, Type::Null])
}

/// `map<string, string|int|float|bool|null>` — type de retour de `headers()`.
/// En pratique toujours peuplée de `string` (un en-tête HTTP est toujours du
/// texte sur le fil) : l'union n'existe que pour la parité de forme avec
/// `params()` (voir docs/roadmap.d/stdlib-httpserver-request-object.md).
fn header_value_ty() -> Type {
    Type::Union(vec![Type::String, Type::Int, Type::Float, Type::Bool, Type::Null])
}

/// `map<string, mixed>` — type de retour de `param()` pour un champ fichier
/// multipart, ou `mixed` seul pour param()/params() eux-mêmes.
fn mixed_map_ty() -> Type {
    Type::Map(Box::new(Type::String), Box::new(Type::Mixed))
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

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTTPServerRequest — objet requête reçu par un handler
//
// Voir docs/roadmap.d/stdlib-httpserver-request-object.md (désormais clos) et
// docs/roadmap.d/stdlib-httpserver-post-body-parsing.md (désormais clos, fondu
// dans ce même chantier). Remplace l'ancien paramètre `req:int` opaque —
// AUCUN changement d'ABI (un paramètre `Type::Named(classe)` se compile déjà
// comme un seul i64, exactement comme `int` — voir
// src/codegen/emit.d/helpers.rs et le précédent `nameless(db:SQLite): void`,
// src/lower/expr.d/nameless.rs).
//
// Toutes les méthodes sont déclarées STATIQUES (comme `HTTPRequest`/`SQLite`)
// mais utilisables en sucre d'instance (`req.path()`) — voir
// `allows_instance_sugar`, src/sema/typecheck.rs.
//
// Méthodes (is_static: true, appelables en sucre `req.methode(...)`) :
//   req.path()                        → string
//   req.method()                      → string
//   req.body()                        → string
//   req.header(name:string)           → string   (recherche insensible à la casse)
//   req.headers()                     → map<string, string|int|float|bool|null>
//   req.query(key:string)             → mixed    (paramètre query string — historique)
//   req.param(key:string, method:string|null = null) → mixed
//   req.params()                      → map<string, map<string, mixed>>
//   req.respond(status:int, body:string)               → void
//   req.respondHeader(name:string, value:string)       → void
//
// Convention runtime : HTTPServerRequest_<method> (implémentation :
// runtime/src/httpserver.rs, section "HTTPServerRequest").
// ─────────────────────────────────────────────────────────────────────────────

pub fn request_class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // req.path() → string
    methods.insert("path".into(), static_m(
        vec![("req", req_ty())],
        Type::String,
    ));

    // req.method() → string
    methods.insert("method".into(), static_m(
        vec![("req", req_ty())],
        Type::String,
    ));

    // req.body() → string
    methods.insert("body".into(), static_m(
        vec![("req", req_ty())],
        Type::String,
    ));

    // req.header(name:string) → string — recherche insensible à la casse.
    methods.insert("header".into(), static_m(
        vec![("req", req_ty()), ("name", Type::String)],
        Type::String,
    ));

    // req.headers() → map<string, string|int|float|bool|null>
    methods.insert("headers".into(), static_m(
        vec![("req", req_ty())],
        Type::Map(Box::new(Type::String), Box::new(header_value_ty())),
    ));

    // req.query(key:string) → mixed — historique (query string uniquement) ;
    // conservé tel quel (déjà `string` de retour avant ce ticket — voir la
    // doc de HTTPServer::query ci-dessus, comportement inchangé).
    methods.insert("query".into(), static_m(
        vec![("req", req_ty()), ("key", Type::String)],
        Type::String,
    ));

    // req.param(key:string, method:string|null = null) → mixed
    methods.insert("param".into(), static_m_opt(
        vec![("req", req_ty()), ("key", Type::String), ("method", method_ty())],
        Type::Mixed,
        2, // req + key requis ; method optionnel (compte SANS le récepteur : voir allows_instance_sugar)
    ));

    // req.params() → map<string, map<string, mixed>>
    methods.insert("params".into(), static_m(
        vec![("req", req_ty())],
        Type::Map(Box::new(Type::String), Box::new(mixed_map_ty())),
    ));

    // req.respond(status:int, body:string) → void
    methods.insert("respond".into(), static_m(
        vec![
            ("req",    req_ty()),
            ("status", Type::Int),
            ("body",   Type::String),
        ],
        Type::Void,
    ));

    // req.respondHeader(name:string, value:string) → void
    methods.insert("respondHeader".into(), static_m(
        vec![
            ("req",   req_ty()),
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
