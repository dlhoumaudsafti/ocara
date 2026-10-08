// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTTPServerSession — état côté serveur des handlers HTTPServer
//
// Fonctions exportées (convention C) :
//
//   HTTPServerRequest_cookie(req, name)          → i64  valeur d'un cookie ("" si absent)
//   HTTPServerRequest_session(req)               → i64  session du visiteur (créée au besoin)
//   HTTPServerSession_id(sess)                   → i64  identifiant de session
//   HTTPServerSession_set(sess, key, val, shape) → void
//   HTTPServerSession_get(sess, key)             → i64  (mixed, null si absent)
//   HTTPServerSession_has(sess, key)             → i64  (bool)
//   HTTPServerSession_remove(sess, key)          → void
//   HTTPServerSession_destroy(sess)              → void  supprime la session et son cookie
//   HTTPServerSession_setGlobal(key, val, shape) → void
//   HTTPServerSession_getGlobal(key)             → i64  (mixed, null si absent)
//   HTTPServerSession_hasGlobal(key)             → i64  (bool)
//   HTTPServerSession_removeGlobal(key)          → void
//
// Identification : cookie `OCARASESSID` (128 bits aléatoires, HttpOnly,
// SameSite=Lax, Path=/), posé à la première utilisation de la session. Un
// identifiant inconnu du serveur n'est jamais adopté (pas de fixation de
// session) : une nouvelle session est créée à la place.
//
// Le handle `HTTPServerSession` est le contexte de la requête lui-même : il
// n'est valide que pendant l'exécution du handler, comme `HTTPServerRequest`.
//
// Stockage : en mémoire, par processus, derrière un `Mutex` unique (sessions
// et état global). Les valeurs sont COPIÉES en profondeur (`Stored`) : la
// valeur Ocara d'origine peut être libérée à la fin du handler, chaque `get`
// en rematérialise une copie neuve. `shape` (argument caché, voir
// `static_leaf_shape`) décrit les feuilles d'un conteneur concret
// (`array<int>`...), restituées brutes. Objets et fonctions : refusés
// (HTTPServerException 102). Aucune expiration : une session vit jusqu'à
// `destroy()` ou l'arrêt du processus.
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use rand::Rng;

use crate::exception::throw_httpserver_exception;
use crate::httpserver::{ctx_ref, header_lookup};
use crate::{alloc_str, ptr_to_str};

pub(crate) const SESSION_COOKIE: &str = "OCARASESSID";
const ERR_SESSION_VALUE: i64 = 102;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Stored {
    Null,
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Array(Vec<Stored>),
    Map(Vec<(String, Stored)>),
}

#[derive(Clone)]
struct Entry {
    value: Stored,
    shape: i64,
}

#[derive(Default)]
struct Store {
    sessions: HashMap<String, HashMap<String, Entry>>,
    globals:  HashMap<String, Entry>,
}

static STORE: Lazy<Mutex<Store>> = Lazy::new(|| Mutex::new(Store::default()));

fn store() -> std::sync::MutexGuard<'static, Store> {
    STORE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ─── Cookies ─────────────────────────────────────────────────────────────────

/// `a=1; b=2` → {a: 1, b: 2} — paires sans `=` ignorées, valeurs non décodées.
pub(crate) fn parse_cookies(header: &str) -> HashMap<String, String> {
    header.split(';')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().trim_matches('"').to_string()))
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

fn request_cookie(req: i64, name: &str) -> Option<String> {
    let ctx = unsafe { ctx_ref(req) };
    header_lookup(&ctx.headers, "Cookie").and_then(|h| parse_cookies(h).remove(name))
}

fn new_session_id() -> String {
    let bytes: [u8; 16] = rand::thread_rng().r#gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn push_set_cookie(req: i64, value: &str) {
    if let Ok(h) = format!("Set-Cookie: {}", value).parse::<tiny_http::Header>() {
        unsafe { ctx_ref(req) }.resp_headers.push(h);
    }
}

/// Session de la requête : reprise depuis le cookie si le serveur la connaît,
/// créée (et son cookie posé) sinon.
fn ensure_session(req: i64) -> String {
    if let Some(id) = unsafe { ctx_ref(req) }.session_id.clone() {
        return id;
    }
    let known = request_cookie(req, SESSION_COOKIE).filter(|id| store().sessions.contains_key(id));
    let id = match known {
        Some(id) => id,
        None => {
            let id = new_session_id();
            store().sessions.insert(id.clone(), HashMap::new());
            push_set_cookie(req, &format!("{}={}; Path=/; HttpOnly; SameSite=Lax", SESSION_COOKIE, id));
            id
        }
    };
    unsafe { ctx_ref(req) }.session_id = Some(id.clone());
    id
}

// ─── Copie profonde des valeurs ──────────────────────────────────────────────

/// Valeur Ocara → copie Rust. `shape` : 0 = `mixed` (valeurs auto-décrites),
/// sinon `kind | depth << 8` — feuilles int/float/bool (kind 1/2/3) BRUTES à
/// `depth` niveaux de conteneur (voir `static_leaf_shape` côté compilateur).
pub(crate) fn capture(val: i64, shape: i64) -> Result<Stored, &'static str> {
    if shape == 0 {
        return capture_mixed(val);
    }
    let (kind, depth) = (shape & 0xff, shape >> 8);
    if depth == 0 {
        return Ok(match kind {
            2 => Stored::Float(f64::from_bits(val as u64)),
            3 => Stored::Bool(val != 0),
            _ => Stored::Int(val),
        });
    }
    let inner = kind | (depth - 1) << 8;
    match unsafe { crate::typecheck::read_tag(val) } {
        crate::typecheck::TAG_ARRAY => capture_array(val, |v| capture(v, inner)),
        crate::typecheck::TAG_MAP => capture_map(val, |v| capture(v, inner)),
        _ => Ok(Stored::Null),
    }
}

fn capture_mixed(val: i64) -> Result<Stored, &'static str> {
    if val == 0 { return Ok(Stored::Null); }
    if crate::is_float_box(val) { return Ok(Stored::Float(unsafe { crate::unbox_float(val) })); }
    if crate::is_bool_box(val) { return Ok(Stored::Bool(unsafe { crate::unbox_bool(val) })); }
    if crate::is_int_box(val) { return Ok(Stored::Int(unsafe { crate::unbox_int(val) })); }
    match crate::get_value_type(val) {
        1 => Ok(Stored::Int(val)),
        4 => Ok(Stored::Str(unsafe { ptr_to_str(val) }.to_string())),
        5 => capture_array(val, capture_mixed),
        6 => capture_map(val, capture_mixed),
        _ => Err("objects and functions cannot be stored in HTTPServerSession"),
    }
}

fn capture_array(val: i64, each: impl Fn(i64) -> Result<Stored, &'static str>) -> Result<Stored, &'static str> {
    (0..crate::__array_len(val)).map(|i| each(crate::__array_get(val, i))).collect::<Result<_, _>>().map(Stored::Array)
}

fn capture_map(val: i64, each: impl Fn(i64) -> Result<Stored, &'static str>) -> Result<Stored, &'static str> {
    let pairs = unsafe { (*(val as *const crate::OcaraMap)).data.clone() };
    pairs.into_iter().map(|(k, v)| each(v).map(|s| (k, s))).collect::<Result<_, _>>().map(Stored::Map)
}

/// Copie Rust → valeur Ocara neuve ; `raw` : feuilles non boxées (conteneur concret).
/// Feuille scalaire : brute dans un conteneur à éléments concrets.
fn is_scalar(s: &Stored) -> bool {
    matches!(s, Stored::Int(_) | Stored::Float(_) | Stored::Bool(_))
}

pub(crate) fn materialize(s: &Stored, raw: bool) -> i64 {
    match s {
        Stored::Null => 0,
        Stored::Int(n) if raw => *n,
        Stored::Float(f) if raw => f.to_bits() as i64,
        Stored::Bool(b) if raw => *b as i64,
        Stored::Int(n) => crate::box_int_if_needed(*n),
        Stored::Float(f) => crate::__box_float(f.to_bits() as i64),
        Stored::Bool(b) => crate::__box_bool(*b as i64),
        Stored::Str(text) => unsafe { alloc_str(text) },
        Stored::Array(items) => {
            let arr = crate::__array_new();
            if raw && items.iter().any(is_scalar) { crate::rc::__rc_mark_raw(arr); }
            items.iter().for_each(|item| crate::array_push_owned(arr, materialize(item, raw)));
            arr
        }
        Stored::Map(pairs) => {
            let map = crate::__map_new();
            if raw && pairs.iter().any(|(_, v)| is_scalar(v)) { crate::rc::__rc_mark_raw(map); }
            for (k, v) in pairs {
                crate::map_set_owned_key(map, unsafe { alloc_str(k) }, materialize(v, raw));
            }
            map
        }
    }
}

fn entry_of(val: i64, shape: i64) -> Entry {
    match capture(val, shape) {
        Ok(value) => Entry { value, shape },
        Err(msg) => unsafe { throw_httpserver_exception(msg, ERR_SESSION_VALUE) },
    }
}

fn key_of(key: i64) -> String {
    unsafe { ptr_to_str(key) }.to_string()
}

fn materialize_entry(entry: Option<Entry>) -> i64 {
    entry.map_or(0, |e| materialize(&e.value, e.shape != 0))
}

// ─── HTTPServerRequest : cookies et accès à la session ───────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerRequest_cookie(req: i64, name: i64) -> i64 {
    let value = request_cookie(req, unsafe { ptr_to_str(name) }).unwrap_or_default();
    unsafe { alloc_str(&value) }
}

#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerRequest_session(req: i64) -> i64 {
    ensure_session(req);
    req
}

// ─── HTTPServerSession : données du visiteur ─────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerSession_id(sess: i64) -> i64 {
    unsafe { alloc_str(&ensure_session(sess)) }
}

#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerSession_set(sess: i64, key: i64, val: i64, shape: i64) {
    let entry = entry_of(val, shape);
    let id = ensure_session(sess);
    store().sessions.entry(id).or_default().insert(key_of(key), entry);
}

#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerSession_get(sess: i64, key: i64) -> i64 {
    let id = ensure_session(sess);
    let entry = store().sessions.get(&id).and_then(|s| s.get(&key_of(key)).cloned());
    materialize_entry(entry)
}

#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerSession_has(sess: i64, key: i64) -> i64 {
    let id = ensure_session(sess);
    store().sessions.get(&id).is_some_and(|s| s.contains_key(&key_of(key))) as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerSession_remove(sess: i64, key: i64) {
    let id = ensure_session(sess);
    if let Some(s) = store().sessions.get_mut(&id) {
        s.remove(&key_of(key));
    }
}

/// Supprime la session et expire son cookie ; un accès ultérieur dans le même
/// handler ouvre une nouvelle session.
#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerSession_destroy(sess: i64) {
    let Some(id) = unsafe { ctx_ref(sess) }.session_id.take()
        .or_else(|| request_cookie(sess, SESSION_COOKIE)) else { return; };
    store().sessions.remove(&id);
    push_set_cookie(sess, &format!("{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0", SESSION_COOKIE));
}

// ─── HTTPServerSession : état global partagé ─────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerSession_setGlobal(key: i64, val: i64, shape: i64) {
    let entry = entry_of(val, shape);
    store().globals.insert(key_of(key), entry);
}

#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerSession_getGlobal(key: i64) -> i64 {
    let entry = store().globals.get(&key_of(key)).cloned();
    materialize_entry(entry)
}

#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerSession_hasGlobal(key: i64) -> i64 {
    store().globals.contains_key(&key_of(key)) as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn HTTPServerSession_removeGlobal(key: i64) {
    store().globals.remove(&key_of(key));
}
