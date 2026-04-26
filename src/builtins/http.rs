// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTTPRequest — classe builtin statique
//
// Toutes les méthodes sont statiques. Les handles `req` et `res` sont des
// entiers opaques (pointeurs gérés par le runtime C).
//
// ── Construction & configuration ────────────────────────────────────────────
//   HTTPRequest::new(url)                 → int   crée une requête
//   HTTPRequest::set_method(req, method)  → void  "GET" | "POST" | "PUT" | …
//   HTTPRequest::set_header(req, k, v)    → void  ajoute un en-tête
//   HTTPRequest::set_body(req, body)      → void  corps (JSON, form, …)
//   HTTPRequest::set_timeout(req, ms)     → void  délai en millisecondes
//
// ── Exécution ────────────────────────────────────────────────────────────────
//   HTTPRequest::send(req)                → int   envoie et retourne une réponse
//
// ── Lecture de la réponse ────────────────────────────────────────────────────
//   HTTPRequest::status(res)              → int            code HTTP (200, 404…)
//   HTTPRequest::body(res)                → string         corps de la réponse
//   HTTPRequest::header(res, name)        → string         valeur d'un en-tête
//   HTTPRequest::headers(res)             → map<str,str>   tous les en-têtes
//   HTTPRequest::ok(res)                  → bool           status 2xx
//   HTTPRequest::is_error(res)            → bool           erreur réseau/timeout
//   HTTPRequest::error(res)               → string         message d'erreur
//
// ── Raccourcis ───────────────────────────────────────────────────────────────
//   HTTPRequest::get(url)                 → int
//   HTTPRequest::post(url, body)          → int
//   HTTPRequest::put(url, body)           → int
//   HTTPRequest::delete(url)              → int
//   HTTPRequest::patch(url, body)         → int
//
// Convention runtime : HTTPRequest_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

fn m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: true,
    }
}

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // ── Construction & configuration ─────────────────────────────────────────

    // HTTPRequest::new(url) → int
    methods.insert("new".into(), m(
        vec![("url", Type::String)],
        Type::Int,
    ));

    // HTTPRequest::set_method(req, method) → void
    methods.insert("set_method".into(), m(
        vec![("req", Type::Int), ("method", Type::String)],
        Type::Void,
    ));

    // HTTPRequest::set_header(req, name, value) → void
    methods.insert("set_header".into(), m(
        vec![("req", Type::Int), ("name", Type::String), ("value", Type::String)],
        Type::Void,
    ));

    // HTTPRequest::set_body(req, body) → void
    methods.insert("set_body".into(), m(
        vec![("req", Type::Int), ("body", Type::String)],
        Type::Void,
    ));

    // HTTPRequest::set_timeout(req, ms) → void
    methods.insert("set_timeout".into(), m(
        vec![("req", Type::Int), ("ms", Type::Int)],
        Type::Void,
    ));

    // ── Exécution ─────────────────────────────────────────────────────────────

    // HTTPRequest::send(req) → int
    methods.insert("send".into(), m(
        vec![("req", Type::Int)],
        Type::Int,
    ));

    // ── Lecture de la réponse ─────────────────────────────────────────────────

    // HTTPRequest::status(res) → int
    methods.insert("status".into(), m(
        vec![("res", Type::Int)],
        Type::Int,
    ));

    // HTTPRequest::body(res) → string
    methods.insert("body".into(), m(
        vec![("res", Type::Int)],
        Type::String,
    ));

    // HTTPRequest::header(res, name) → string
    methods.insert("header".into(), m(
        vec![("res", Type::Int), ("name", Type::String)],
        Type::String,
    ));

    // HTTPRequest::headers(res) → map<string, string>
    methods.insert("headers".into(), m(
        vec![("res", Type::Int)],
        Type::Map(Box::new(Type::String), Box::new(Type::String)),
    ));

    // HTTPRequest::ok(res) → bool  (status >= 200 && < 300)
    methods.insert("ok".into(), m(
        vec![("res", Type::Int)],
        Type::Bool,
    ));

    // HTTPRequest::is_error(res) → bool  (erreur réseau ou timeout)
    methods.insert("is_error".into(), m(
        vec![("res", Type::Int)],
        Type::Bool,
    ));

    // HTTPRequest::error(res) → string  ("" si aucune erreur)
    methods.insert("error".into(), m(
        vec![("res", Type::Int)],
        Type::String,
    ));

    // ── Raccourcis ────────────────────────────────────────────────────────────

    // HTTPRequest::get(url) → int
    methods.insert("get".into(), m(
        vec![("url", Type::String)],
        Type::Int,
    ));

    // HTTPRequest::post(url, body) → int
    methods.insert("post".into(), m(
        vec![("url", Type::String), ("body", Type::String)],
        Type::Int,
    ));

    // HTTPRequest::put(url, body) → int
    methods.insert("put".into(), m(
        vec![("url", Type::String), ("body", Type::String)],
        Type::Int,
    ));

    // HTTPRequest::delete(url) → int
    methods.insert("delete".into(), m(
        vec![("url", Type::String)],
        Type::Int,
    ));

    // HTTPRequest::patch(url, body) → int
    methods.insert("patch".into(), m(
        vec![("url", Type::String), ("body", Type::String)],
        Type::Int,
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
