//! `UIEvent` : valeur native comptée remise aux gestionnaires `ui.on`
//! (`event.action()`, `event.id()`, `event.value()`, `event.index()`).

use serde_json::Value;

use crate::ffi::{__native_data, __native_new, from_json, string};

pub struct Event {
    pub action: String,
    pub id: Option<String>,
    /// Instance de composant d'où vient une action propre, `""` sinon.
    pub instance: String,
    pub value: Value,
    pub index: Option<i64>,
}

extern "C" fn drop_event(data: i64) {
    drop(unsafe { Box::from_raw(data as *mut Event) });
}

/// Nouvelle valeur `UIEvent` (compte 1).
pub fn new(event: Event) -> i64 {
    unsafe { __native_new(Box::into_raw(Box::new(event)) as i64, drop_event as extern "C" fn(i64) as usize as i64) }
}

fn with<R>(val: i64, f: impl FnOnce(&Event) -> R, default: R) -> R {
    let data = unsafe { __native_data(val, drop_event as extern "C" fn(i64) as usize as i64) };
    if data == 0 { default } else { f(unsafe { &*(data as *const Event) }) }
}

/// `event.action()` → `string` : action déclarée dans le `.oc.ui`.
#[unsafe(no_mangle)]
pub extern "C" fn UIEvent_action(event: i64) -> i64 {
    with(event, |e| string(&e.action), 0)
}

/// `event.id()` → `string` : `id` du nœud, `""` s'il n'en a pas.
#[unsafe(no_mangle)]
pub extern "C" fn UIEvent_id(event: i64) -> i64 {
    with(event, |e| string(e.id.as_deref().unwrap_or("")), 0)
}

/// `event.value()` → `mixed` : texte saisi, case cochée, nombre, élément de
/// liste (`map`)… ; `null` pour un clic.
#[unsafe(no_mangle)]
pub extern "C" fn UIEvent_value(event: i64) -> i64 {
    with(event, |e| from_json(&e.value), 0)
}

/// `event.instance()` → `string` : `id` de l'instance du composant dont vient
/// l'action propre (`ui.component(…).on`), `""` sinon.
#[unsafe(no_mangle)]
pub extern "C" fn UIEvent_instance(event: i64) -> i64 {
    with(event, |e| string(&e.instance), 0)
}

/// `event.index()` → `int` : rang de l'élément choisi (`onSelect`), -1 sinon.
#[unsafe(no_mangle)]
pub extern "C" fn UIEvent_index(event: i64) -> i64 {
    with(event, |e| e.index.unwrap_or(-1), -1)
}
