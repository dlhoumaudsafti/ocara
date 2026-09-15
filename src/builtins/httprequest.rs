// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTTPRequest / ocara.HTTPResponse — classes builtin
//
// Toutes les méthodes (des deux classes) sont déclarées statiques sur
// `HTTPRequest` — `HTTPResponse` n'existe que comme type NOMMÉ opaque (voir
// `response_class` plus bas), pour que `scoped`/`consumed res:HTTPResponse`
// soit exprimable et que le handle soit reconnu comme une vraie ressource
// (voir `OwnershipClass::Resource`, `src/sema/scope.rs`) — auparavant
// `int`, ce qui rendait impossible de suivre sa fermeture obligatoire
// (`close`/`closeResponse`, déjà présents côté runtime, jamais reliés au
// système de possession). Voir docs/roadmap.d/memoire-double-free-et-fuites-scoped.md.
//
// ── Construction & configuration ────────────────────────────────────────────
//   HTTPRequest::new(url)                 → HTTPRequest   crée une requête
//   HTTPRequest::setMethod(req, method)  → void  "GET" | "POST" | "PUT" | …
//   HTTPRequest::setHeader(req, k, v)    → void  ajoute un en-tête
//   HTTPRequest::setBody(req, body)      → void  corps (JSON, form, …)
//   HTTPRequest::setTimeout(req, ms)     → void  délai en millisecondes
//
// ── Exécution ────────────────────────────────────────────────────────────────
//   HTTPRequest::send(req)                → HTTPResponse   envoie et retourne une réponse
//
// ── Lecture de la réponse ────────────────────────────────────────────────────
//   HTTPRequest::status(res)              → int            code HTTP (200, 404…)
//   HTTPRequest::body(res)                → string         corps de la réponse
//   HTTPRequest::header(res, name)        → string         valeur d'un en-tête
//   HTTPRequest::headers(res)             → map<str,str>   tous les en-têtes
//   HTTPRequest::ok(res)                  → bool           status 2xx
//   HTTPRequest::isError(res)            → bool           erreur réseau/timeout
//   HTTPRequest::error(res)               → string         message d'erreur
//
// ── Raccourcis ───────────────────────────────────────────────────────────────
//   HTTPRequest::get(url)                 → HTTPResponse
//   HTTPRequest::post(url, body)          → HTTPResponse
//   HTTPRequest::put(url, body)           → HTTPResponse
//   HTTPRequest::delete(url)              → HTTPResponse
//   HTTPRequest::patch(url, body)         → HTTPResponse
//
// Convention runtime : HTTPRequest_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

fn req_ty() -> Type { Type::Named("HTTPRequest".to_string()) }
fn res_ty() -> Type { Type::Named("HTTPResponse".to_string()) }

fn m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
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

    // ── Construction & configuration ─────────────────────────────────────────

    // HTTPRequest::new(url) → HTTPRequest
    methods.insert("new".into(), m(
        vec![("url", Type::String)],
        req_ty(),
    ));

    // HTTPRequest::setMethod(req, method) → void
    methods.insert("setMethod".into(), m(
        vec![("req", req_ty()), ("method", Type::String)],
        Type::Void,
    ));

    // HTTPRequest::setHeader(req, name, value) → void
    methods.insert("setHeader".into(), m(
        vec![("req", req_ty()), ("name", Type::String), ("value", Type::String)],
        Type::Void,
    ));

    // HTTPRequest::setBody(req, body) → void
    methods.insert("setBody".into(), m(
        vec![("req", req_ty()), ("body", Type::String)],
        Type::Void,
    ));

    // HTTPRequest::setTimeout(req, ms) → void
    methods.insert("setTimeout".into(), m(
        vec![("req", req_ty()), ("ms", Type::Int)],
        Type::Void,
    ));

    // ── Exécution ─────────────────────────────────────────────────────────────

    // HTTPRequest::send(req) → HTTPResponse
    methods.insert("send".into(), m(
        vec![("req", req_ty())],
        res_ty(),
    ));

    // ── Lecture de la réponse ─────────────────────────────────────────────────

    // HTTPRequest::status(res) → int
    methods.insert("status".into(), m(
        vec![("res", res_ty())],
        Type::Int,
    ));

    // HTTPRequest::body(res) → string
    methods.insert("body".into(), m(
        vec![("res", res_ty())],
        Type::String,
    ));

    // HTTPRequest::header(res, name) → string
    methods.insert("header".into(), m(
        vec![("res", res_ty()), ("name", Type::String)],
        Type::String,
    ));

    // HTTPRequest::headers(res) → map<string, string>
    methods.insert("headers".into(), m(
        vec![("res", res_ty())],
        Type::Map(Box::new(Type::String), Box::new(Type::String)),
    ));

    // HTTPRequest::ok(res) → bool  (status >= 200 && < 300)
    methods.insert("ok".into(), m(
        vec![("res", res_ty())],
        Type::Bool,
    ));

    // HTTPRequest::isError(res) → bool  (erreur réseau ou timeout)
    methods.insert("isError".into(), m(
        vec![("res", res_ty())],
        Type::Bool,
    ));

    // HTTPRequest::error(res) → string  ("" si aucune erreur)
    methods.insert("error".into(), m(
        vec![("res", res_ty())],
        Type::String,
    ));

    // ── Raccourcis ────────────────────────────────────────────────────────────

    // HTTPRequest::get(url) → HTTPResponse
    methods.insert("get".into(), m(
        vec![("url", Type::String)],
        res_ty(),
    ));

    // HTTPRequest::post(url, body) → HTTPResponse
    methods.insert("post".into(), m(
        vec![("url", Type::String), ("body", Type::String)],
        res_ty(),
    ));

    // HTTPRequest::put(url, body) → HTTPResponse
    methods.insert("put".into(), m(
        vec![("url", Type::String), ("body", Type::String)],
        res_ty(),
    ));

    // HTTPRequest::delete(url) → HTTPResponse
    methods.insert("delete".into(), m(
        vec![("url", Type::String)],
        res_ty(),
    ));

    // HTTPRequest::patch(url, body) → HTTPResponse
    methods.insert("patch".into(), m(
        vec![("url", Type::String), ("body", Type::String)],
        res_ty(),
    ));

    // HTTPRequest::close(req) → void — libère un handle de `new` (Box::from_raw)
    methods.insert("close".into(), m(
        vec![("req", req_ty())],
        Type::Void,
    ));

    // HTTPRequest::closeResponse(res) → void — libère un handle de `send`/
    // `get`/`post`/`put`/`delete`/`patch` (struct différente, fonction dédiée)
    methods.insert("closeResponse".into(), m(
        vec![("res", res_ty())],
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

/// `HTTPResponse` : type nommé opaque sans méthode propre — toutes les
/// opérations sur une réponse restent des méthodes STATIQUES de
/// `HTTPRequest` (`status(res)`, `body(res)`...), comme avant. Seul le NOM
/// existe ici, pour que `scoped`/`consumed res:HTTPResponse` soit une
/// annotation de type valide et que le handle soit reconnu comme une
/// ressource (voir `ownership_class`, `src/sema/scope.rs`).
pub fn response_class() -> ClassInfo {
    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods:      HashMap::new(),
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}
