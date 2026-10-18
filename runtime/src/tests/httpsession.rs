// ─────────────────────────────────────────────────────────────────────────────
// Tests unitaires — ocara.HTTPServerSession : lecture du header `Cookie`,
// copie profonde des valeurs (`capture`/`materialize`) et état global.
// Le cycle cookie bout-en-bout est couvert par
// examples/tests/73_httpserver_sessionTest.oc.
// ─────────────────────────────────────────────────────────────────────────────

use crate::httpsession::{capture, materialize, parse_cookies, set_session_timeout, Session, Store, Stored};
use crate::httpsession::{HTTPServerSession_getGlobal, HTTPServerSession_hasGlobal, HTTPServerSession_removeGlobal, HTTPServerSession_setGlobal};
use crate::*;

#[test]
fn parse_cookies_splits_pairs_and_trims() {
    let c = parse_cookies("a=1; OCARASESSID=abc ;  theme=\"dark\"; broken");
    assert_eq!(c.get("a").map(String::as_str), Some("1"));
    assert_eq!(c.get("OCARASESSID").map(String::as_str), Some("abc"));
    assert_eq!(c.get("theme").map(String::as_str), Some("dark"));
    assert!(!c.contains_key("broken"));
}

#[test]
fn mixed_scalars_round_trip() {
    for (val, expected) in [
        (box_int_if_needed(0), Stored::Int(0)),
        (box_int_if_needed(1 << 40), Stored::Int(1 << 40)),
        (__box_float(2.5f64.to_bits() as i64), Stored::Float(2.5)),
        (__box_bool(1), Stored::Bool(true)),
        (0, Stored::Null),
    ] {
        let stored = capture(val, 0).unwrap();
        assert_eq!(stored, expected);
        assert_eq!(capture(materialize(&stored, false), 0).unwrap(), expected);
    }
}

#[test]
fn concrete_int_array_keeps_raw_leaves() {
    let arr = crate::rc::__rc_mark_raw(__array_new());
    for n in [0i64, 1, 70000] { __array_push(arr, n); }
    let stored = capture(arr, 1 | 1 << 8).unwrap();
    assert_eq!(stored, Stored::Array(vec![Stored::Int(0), Stored::Int(1), Stored::Int(70000)]));
    let copy = materialize(&stored, true);
    assert_ne!(copy, arr, "une copie neuve, jamais l'original");
    assert_eq!((0..3).map(|i| __array_get(copy, i)).collect::<Vec<_>>(), vec![0, 1, 70000]);
}

#[test]
fn nested_mixed_map_round_trip() {
    let inner = __array_new();
    __array_push(inner, unsafe { alloc_str("x") });
    let map = __map_new();
    unsafe {
        __map_set(map, alloc_str("name"), alloc_str("Ada"));
        __map_set(map, alloc_str("tags"), inner);
    }
    let stored = capture(map, 0).unwrap();
    assert_eq!(stored, Stored::Map(vec![
        ("name".into(), Stored::Str("Ada".into())),
        ("tags".into(), Stored::Array(vec![Stored::Str("x".into())])),
    ]));
    assert_eq!(capture(materialize(&stored, false), 0).unwrap(), stored);
}

#[test]
fn globals_set_has_get_remove() {
    let key = unsafe { alloc_str("tests_httpsession_counter") };
    assert_eq!(HTTPServerSession_hasGlobal(key), 0);
    HTTPServerSession_setGlobal(key, box_int_if_needed(0), 0);
    assert_eq!(HTTPServerSession_hasGlobal(key), 1);
    assert_eq!(capture(HTTPServerSession_getGlobal(key), 0).unwrap(), Stored::Int(0));
    HTTPServerSession_removeGlobal(key);
    assert_eq!(HTTPServerSession_hasGlobal(key), 0);
    assert_eq!(HTTPServerSession_getGlobal(key), 0);
}

// ─── Expiration des sessions ────────────────────────────────────────────────

fn store_with(id: &str, idle: std::time::Duration, now: std::time::Instant) -> Store {
    let mut store = Store::default();
    let mut session = Session::default();
    session.last_access = now - idle;
    store.sessions.insert(id.to_string(), session);
    store
}

#[test]
fn active_session_is_resumed_and_touched() {
    let now = std::time::Instant::now() + std::time::Duration::from_secs(3600);
    let mut store = store_with("a", std::time::Duration::from_secs(60), now);
    assert!(store.resume("a", now));
    assert_eq!(store.sessions["a"].last_access, now);
}

// Inactive depuis plus de 30 min (défaut) : jamais reprise, retirée.
#[test]
fn expired_session_is_not_resumed() {
    let now = std::time::Instant::now() + std::time::Duration::from_secs(7200);
    let mut store = store_with("a", std::time::Duration::from_secs(31 * 60), now);
    assert!(!store.resume("a", now));
    assert!(store.sessions.is_empty());
    assert!(!store.resume("inconnue", now));
}

// Purge au plus une fois par minute ; ne retire que les sessions expirées.
#[test]
fn purge_removes_expired_sessions_at_most_once_a_minute() {
    let now = std::time::Instant::now() + std::time::Duration::from_secs(7200);
    let mut store = store_with("vieille", std::time::Duration::from_secs(31 * 60), now);
    let mut active = Session::default();
    active.last_access = now;
    store.sessions.insert("active".to_string(), active);
    store.last_purge = now;
    store.purge_expired(now);
    assert_eq!(store.sessions.len(), 2);
    store.last_purge = now - std::time::Duration::from_secs(61);
    store.purge_expired(now);
    assert!(store.sessions.contains_key("active"));
    assert!(!store.sessions.contains_key("vieille"));
}

#[test]
fn zero_timeout_disables_expiration() {
    let now = std::time::Instant::now() + std::time::Duration::from_secs(7200);
    let mut store = store_with("a", std::time::Duration::from_secs(3600), now);
    set_session_timeout(0);
    let resumed = store.resume("a", now);
    set_session_timeout(30 * 60);
    assert!(resumed);
}
