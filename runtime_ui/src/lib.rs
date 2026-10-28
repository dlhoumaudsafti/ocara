//! Partie commune des backends d'`ocara.UI` (docs/oc-ui.md) : chaque objet
//! `UI` (clé : son pointeur) garde l'arbre normalisé de son `.oc.ui`, l'état
//! de ses nœuds et ses gestionnaires `ui.on`. Les moteurs de rendu
//! (runtime_tauri/src/ui/engine.js, OcaraUIEngine.kt) remettent leurs
//! messages à [`post`] ; les actions sont traitées sur un thread Ocara dédié,
//! l'une après l'autre.

pub mod event;
pub mod ffi;

use std::collections::HashMap;
use std::sync::{mpsc, Mutex, OnceLock};

use serde_json::{Map, Value};

use event::Event;
use ffi::{raise, string, to_json};

/// Codes d'erreur `UIException` (docs/oc-ui.md).
pub const ERR_ACTION: i64 = 101;
pub const ERR_ID: i64 = 102;
pub const ERR_SCREEN: i64 = 103;
pub const ERR_UNAVAILABLE: i64 = 104;
pub const ERR_TREE: i64 = 105;

struct Ui {
    tree: Value,
    /// Nœuds qui ont un `id` (propriétés déclarées).
    nodes: HashMap<String, Map<String, Value>>,
    /// État posé par `set` ou par l'utilisateur, par `id` puis propriété.
    state: HashMap<String, Map<String, Value>>,
    handlers: HashMap<String, Vec<i64>>,
}

static UIS: Mutex<Option<HashMap<i64, Ui>>> = Mutex::new(None);

fn with<R>(this: i64, f: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    UIS.lock().unwrap().as_mut()?.get_mut(&this).map(f)
}

/// Action prise en charge par la plateforme quand aucun `ui.on` ne la lie
/// (`close`, `back` sur l'écran de départ), sur le thread des actions.
pub type DefaultAction = fn(this: i64, action: &str);

static DEFAULT_ACTION: OnceLock<DefaultAction> = OnceLock::new();

fn collect_nodes(node: &Value, out: &mut HashMap<String, Map<String, Value>>) {
    let Some(object) = node.as_object() else { return };
    if let Some(id) = object.get("id").and_then(Value::as_str) {
        out.insert(id.to_string(), object.clone());
    }
    for child in object.get("children").and_then(Value::as_array).into_iter().flatten() {
        collect_nodes(child, out);
    }
    // Les nœuds du gabarit `item` d'une liste se répètent : pas d'état propre.
}

/// `use UI(…)` : enregistre l'arbre (JSON produit par le compilateur).
pub fn init(this: i64, tree_json: &str, default_action: DefaultAction) -> Value {
    let _ = DEFAULT_ACTION.set(default_action);
    let tree: Value = match serde_json::from_str(tree_json) {
        Ok(tree) => tree,
        Err(e) => raise(ERR_TREE, format!("arbre d'interface invalide : {e}")),
    };
    let mut nodes = HashMap::new();
    for screen in tree.get("screens").and_then(Value::as_object).into_iter().flat_map(|s| s.values()) {
        collect_nodes(&screen["layout"], &mut nodes);
    }
    let ui = Ui { tree: tree.clone(), nodes, state: HashMap::new(), handlers: HashMap::new() };
    UIS.lock().unwrap().get_or_insert_with(HashMap::new).insert(this, ui);
    tree
}

/// Libération de l'objet `UI` : gestionnaires rendus.
pub fn release(this: i64) {
    let removed = UIS.lock().unwrap().as_mut().and_then(|uis| uis.remove(&this));
    for &handler in removed.iter().flat_map(|ui| ui.handlers.values().flatten()) {
        unsafe { ffi::__rc_release(handler) };
    }
}

pub fn tree(this: i64) -> Option<Value> {
    with(this, |ui| ui.tree.clone())
}

/// `ui.on(action, handler)` : action déclarée par le `.oc.ui`, ou `back`,
/// `close`, `ready`.
pub fn on(this: i64, action: &str, handler: i64) {
    let known = with(this, |ui| {
        let declared = ui.tree["actions"].as_array().is_some_and(|a| a.iter().any(|v| v == action));
        declared || matches!(action, "back" | "close" | "ready")
    }).unwrap_or(false);
    if !known {
        let declared = tree(this).map(|t| t["actions"].to_string()).unwrap_or_default();
        raise(ERR_ACTION, format!("action '{action}' absente du .oc.ui (actions : {declared})"));
    }
    unsafe { ffi::__rc_retain(handler) };
    with(this, |ui| ui.handlers.entry(action.to_string()).or_default().push(handler));
}

fn check_id(this: i64, id: &str) {
    let known = with(this, |ui| ui.nodes.contains_key(id)).unwrap_or(false);
    if !known {
        let mut ids: Vec<String> = with(this, |ui| ui.nodes.keys().cloned().collect()).unwrap_or_default();
        ids.sort();
        raise(ERR_ID, format!("id '{id}' absent du .oc.ui (id : {})", ids.join(", ")));
    }
}

/// `ui.get(id, prop)` : valeur posée, sinon celle du `.oc.ui`, sinon `null`.
pub fn get(this: i64, id: &str, prop: &str) -> i64 {
    check_id(this, id);
    let value = with(this, |ui| {
        ui.state.get(id).and_then(|s| s.get(prop))
            .or_else(|| ui.nodes.get(id).and_then(|n| n.get(prop)))
            .cloned()
    }).flatten().unwrap_or(Value::Null);
    ffi::from_json(&value)
}

/// `ui.set(id, prop, valeur)` : garde la valeur et la rend en JSON, à
/// transmettre au moteur.
pub fn set(this: i64, id: &str, prop: &str, value: i64) -> Value {
    check_id(this, id);
    let value = to_json(value);
    set_json(this, id, prop, value.clone());
    value
}

fn set_json(this: i64, id: &str, prop: &str, value: Value) {
    with(this, |ui| { ui.state.entry(id.to_string()).or_default().insert(prop.to_string(), value); });
}

/// État gardé (`{ id: { prop: valeur } }`), rendu au moteur quand sa page
/// ou son Activity est recréée.
pub fn state(this: i64) -> Value {
    let state = with(this, |ui| ui.state.clone()).unwrap_or_default();
    Value::Object(state.into_iter().map(|(id, props)| (id, Value::Object(props))).collect())
}

/// `ui.navigate(screen)` : écran déclaré.
pub fn check_screen(this: i64, screen: &str) {
    let known = with(this, |ui| ui.tree["screens"].get(screen).is_some()).unwrap_or(false);
    if !known {
        raise(ERR_SCREEN, format!("écran '{screen}' absent du .oc.ui"));
    }
}

/// Message d'un moteur : `{ id, prop, value }` (valeur changée par
/// l'utilisateur) ou `{ action, id, value, index }`.
pub fn post(this: i64, message: &Value) {
    let id = message.get("id").and_then(Value::as_str);
    if let (Some(id), Some(prop)) = (id, message.get("prop").and_then(Value::as_str)) {
        set_json(this, id, prop, message.get("value").cloned().unwrap_or(Value::Null));
        return;
    }
    let Some(action) = message.get("action").and_then(Value::as_str) else { return };
    let event = Event {
        action: action.to_string(),
        id: id.map(str::to_string),
        value: message.get("value").cloned().unwrap_or(Value::Null),
        index: message.get("index").and_then(Value::as_i64),
    };
    let queue = QUEUE.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(i64, Event)>();
        std::thread::spawn(move || {
            for (this, event) in rx {
                dispatch(this, event);
            }
        });
        Mutex::new(tx)
    });
    let _ = queue.lock().unwrap().send((this, event));
}

static QUEUE: OnceLock<Mutex<mpsc::Sender<(i64, Event)>>> = OnceLock::new();

/// Appelle les gestionnaires de l'action (thread des actions).
fn dispatch(this: i64, event: Event) {
    let handlers = with(this, |ui| ui.handlers.get(&event.action).cloned().unwrap_or_default()).unwrap_or_default();
    if handlers.is_empty() {
        if let Some(default) = DEFAULT_ACTION.get() {
            default(this, &event.action);
        }
        return;
    }
    let action = event.action.clone();
    unsafe {
        ffi::__rc_thread_enter();
        let value = event::new(event);
        for handler in handlers {
            let uncaught = ffi::__call_closure2_catching(handler, this, value);
            if uncaught != 0 {
                eprintln!("UI : gestionnaire de '{action}' : exception non rattrapée ({})", ffi::str_arg(uncaught));
                ffi::__rc_release(uncaught);
            }
        }
        ffi::__rc_release(value);
        ffi::__rc_thread_leave();
    }
}

/// Texte Ocara possédé (pour les plateformes).
pub fn ocara_string(text: &str) -> i64 {
    string(text)
}
