//! Partie commune des backends d'`ocara.UI` (docs/oc-ui.md) : chaque objet
//! `UI` (clé : son pointeur) garde l'arbre normalisé de son `.oc.ui`, l'état
//! de ses nœuds et ses gestionnaires `ui.on`. Les moteurs de rendu
//! (runtime_tauri/src/ui/engine.js, OcaraUIEngine.kt) remettent leurs
//! messages à [`post`] ; les actions sont traitées sur un thread Ocara dédié,
//! l'une après l'autre.

mod bind;
pub mod component;
mod form;
pub mod event;
pub mod ffi;
mod vars;

use std::collections::HashMap;
use std::sync::{mpsc, Mutex, OnceLock};

use serde_json::{Map, Value};

pub use bind::{bind, unbind, SendSet};
pub use form::validate;
use event::Event;
use ffi::{raise, string, to_json};

/// Codes d'erreur `UIException` (docs/oc-ui.md).
pub const ERR_ACTION: i64 = 101;
pub const ERR_ID: i64 = 102;
pub const ERR_SCREEN: i64 = 103;
pub const ERR_UNAVAILABLE: i64 = 104;
pub const ERR_TREE: i64 = 105;
pub const ERR_ROOT: i64 = 107;
pub const ERR_ANIMATION: i64 = 108;

struct Ui {
    /// Arbre du compilateur (textes avec `${nom}`) et variables.
    raw: Value,
    vars: Map<String, Value>,
    /// Arbre affiché : textes remplis.
    tree: Value,
    /// Nœuds qui ont un `id` (propriétés déclarées), toutes variantes.
    nodes: HashMap<String, Map<String, Value>>,
    /// État posé par `set` ou par l'utilisateur, par `id` puis propriété.
    state: HashMap<String, Map<String, Value>>,
    handlers: HashMap<String, Vec<i64>>,
    /// Classe de taille rapportée par le moteur (action `resize`).
    size_class: String,
    /// Racines liées par `ui.bind` (voir bind.rs).
    bindings: HashMap<String, bind::Binding>,
    /// Dernière valeur de l'objet lue pour chaque nœud lié (`id`, propriété).
    seen: HashMap<(String, String), Value>,
}

static UIS: Mutex<Option<HashMap<i64, Ui>>> = Mutex::new(None);

fn with<R>(this: i64, f: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    UIS.lock().unwrap().as_mut()?.get_mut(&this).map(f)
}

/// Action prise en charge par la plateforme quand aucun `ui.on` ne la lie
/// (`close`, `back` sur l'écran de départ), sur le thread des actions.
pub type DefaultAction = fn(this: i64, action: &str);

static DEFAULT_ACTION: OnceLock<DefaultAction> = OnceLock::new();
static SEND_SET: OnceLock<SendSet> = OnceLock::new();

fn collect_nodes(node: &Value, out: &mut HashMap<String, Map<String, Value>>) {
    let Some(object) = node.as_object() else { return };
    if let Some(id) = object.get("id").and_then(Value::as_str) {
        out.entry(id.to_string()).or_insert_with(|| object.clone());
    }
    for child in object.get("children").and_then(Value::as_array).into_iter().flatten() {
        collect_nodes(child, out);
    }
    // Les nœuds du gabarit `item` d'une liste se répètent : pas d'état propre.
}

/// Racines des nœuds d'un arbre : disposition de chaque écran, barre de
/// titre d'un écran (`bar`) et celle de tous les écrans.
pub(crate) fn roots(tree: &Value) -> Vec<&Value> {
    let mut out = Vec::new();
    for screen in tree.get("screens").and_then(Value::as_object).into_iter().flat_map(|s| s.values()) {
        out.push(&screen["layout"]);
        out.extend(screen.get("bar"));
    }
    out.extend(tree.get("bar"));
    out
}

/// Nœuds à `id` de l'arbre et de ses variantes de taille.
fn nodes_of(tree: &Value) -> HashMap<String, Map<String, Value>> {
    let mut nodes = HashMap::new();
    let variants = tree.get("variants").and_then(Value::as_object).into_iter().flat_map(|v| v.values());
    for t in std::iter::once(tree).chain(variants) {
        for root in roots(t) {
            collect_nodes(root, &mut nodes);
        }
    }
    nodes
}

fn vars_of(vars: Value) -> Map<String, Value> {
    match vars {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn warn_missing(raw: &Value, vars: &Map<String, Value>) {
    let missing = vars::missing(raw, vars);
    if !missing.is_empty() {
        eprintln!("UI : variable(s) sans valeur, affichée(s) telle(s) quelle(s) : {}", missing.join(", "));
    }
}

/// `use UI(…)` : enregistre l'arbre (JSON produit par le compilateur) et
/// ses variables ; rend l'arbre aux textes remplis. `send_set` remet au
/// moteur une valeur venue d'un objet lié.
pub fn init(this: i64, tree_json: &str, vars: Value, default_action: DefaultAction, send_set: SendSet) -> Value {
    let _ = DEFAULT_ACTION.set(default_action);
    let _ = SEND_SET.set(send_set);
    let raw: Value = match serde_json::from_str(tree_json) {
        Ok(tree) => tree,
        Err(e) => raise(ERR_TREE, format!("arbre d'interface invalide : {e}")),
    };
    let vars = vars_of(vars);
    warn_missing(&raw, &vars);
    let tree = vars::resolve_tree(&raw, &vars);
    let ui = Ui {
        nodes: nodes_of(&tree),
        raw,
        vars,
        tree: tree.clone(),
        state: HashMap::new(),
        handlers: HashMap::new(),
        size_class: String::new(),
        bindings: HashMap::new(),
        seen: HashMap::new(),
    };
    UIS.lock().unwrap().get_or_insert_with(HashMap::new).insert(this, ui);
    tree
}

fn rebuild(ui: &mut Ui) -> Value {
    ui.tree = vars::resolve_tree(&ui.raw, &ui.vars);
    ui.nodes = nodes_of(&ui.tree);
    ui.tree.clone()
}

/// Rechargement à chaud : nouvel arbre, variables, état et gestionnaires
/// gardés ; rend l'arbre affiché.
pub fn reload(this: i64, raw: Value) -> Value {
    with(this, |ui| {
        warn_missing(&raw, &ui.vars);
        ui.raw = raw;
        rebuild(ui)
    }).unwrap_or(Value::Null)
}

/// `ui.setVars(vars)` : nouvelles valeurs (ajoutées aux précédentes) ;
/// rend l'arbre affiché.
pub fn set_vars(this: i64, vars: Value) -> Value {
    with(this, |ui| {
        ui.vars.extend(vars_of(vars));
        rebuild(ui)
    }).unwrap_or(Value::Null)
}

/// Texte d'un message (`toast`, `alert`, `confirm`, `notify`) : variables
/// `${nom}` remplies.
pub fn text(this: i64, text: &str) -> String {
    with(this, |ui| vars::resolve(&Value::String(text.to_string()), &ui.vars))
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| text.to_string())
}

/// `ui.sizeClass()` : classe rapportée par le moteur, `""` avant l'affichage.
pub fn size_class(this: i64) -> String {
    with(this, |ui| ui.size_class.clone()).unwrap_or_default()
}

/// `ui.has(id)` : le nœud `id` est-il dans la variante affichée (celle de
/// la classe rapportée par le moteur ; l'arbre par défaut avant
/// l'affichage) ? Les gabarits `item` des listes comptent.
pub fn has(this: i64, id: &str) -> bool {
    with(this, |ui| {
        roots(displayed(&ui.tree, &ui.size_class)).into_iter().any(|root| declares(root, id))
    }).unwrap_or(false)
}

/// Arbre affiché pour la classe `class` : la plus grande variante dont le
/// seuil ne dépasse pas celui de la classe (même règle que les moteurs).
fn displayed<'t>(tree: &'t Value, class: &str) -> &'t Value {
    let sizes = tree.get("sizes").and_then(Value::as_object);
    let width = sizes.and_then(|s| s.get(class)).and_then(Value::as_u64);
    let (Some(sizes), Some(width), Some(variants)) = (sizes, width, tree.get("variants").and_then(Value::as_object)) else { return tree };
    variants.iter()
        .filter_map(|(c, v)| sizes.get(c).and_then(Value::as_u64).filter(|w| *w <= width).map(|w| (w, v)))
        .max_by_key(|(w, _)| *w)
        .map(|(_, v)| v)
        .unwrap_or(tree)
}

/// Le nœud ou un de ses descendants (enfants, gabarit `item`) a l'`id`.
fn declares(node: &Value, id: &str) -> bool {
    node.get("id").and_then(Value::as_str) == Some(id)
        || node.get("children").and_then(Value::as_array).into_iter().flatten().any(|c| declares(c, id))
        || node.get("item").is_some_and(|item| declares(item, id))
}

/// Libération de l'objet `UI` : son état natif.
pub fn release(this: i64) {
    // Gestionnaires : rendus avec le champ `__kept` de l'objet (`on`).
    UIS.lock().unwrap().as_mut().and_then(|uis| uis.remove(&this));
}

pub fn tree(this: i64) -> Option<Value> {
    with(this, |ui| ui.tree.clone())
}

/// `ui.on(action, handler)` : action déclarée par le `.oc.ui`, ou `back`,
/// `close`, `ready`, `resize`.
pub fn on(this: i64, action: &str, handler: i64) {
    let known = with(this, |ui| {
        let declared = ui.tree["actions"].as_array().is_some_and(|a| a.iter().any(|v| v == action));
        declared || matches!(action, "back" | "close" | "ready" | "resize")
    }).unwrap_or(false);
    if !known {
        let declared = tree(this).map(|t| t["actions"].to_string()).unwrap_or_default();
        raise(ERR_ACTION, format!("action '{action}' absente du .oc.ui (actions : {declared})"));
    }
    // Référence gardée par l'objet (`__kept`), pas par l'état natif : un
    // gestionnaire qui capture `ui` forme un cycle visible du collecteur.
    if with(this, |ui| ui.handlers.entry(action.to_string()).or_default().push(handler)).is_some() {
        unsafe { ffi::__rc_keep(this, 1, handler) };
    }
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

/// Propriétés effectives du nœud `id` : celles du `.oc.ui`, puis l'état posé.
pub fn props(this: i64, id: &str) -> Map<String, Value> {
    with(this, |ui| {
        let mut props = ui.nodes.get(id).cloned().unwrap_or_default();
        if let Some(state) = ui.state.get(id) {
            props.extend(state.clone());
        }
        props
    }).unwrap_or_default()
}

/// Déclenche l'action liée à `event` (`onLoad`…) du nœud `id`, s'il en a une.
pub fn fire(this: i64, id: &str, event: &str, value: Value) {
    if let Some(Value::String(action)) = props(this, id).get(event) {
        post(this, &serde_json::json!({ "action": action, "id": id, "value": value }));
    }
}

/// `ui.set(id, prop, valeur)` : garde la valeur (variables `${nom}`
/// remplies) et la rend en JSON, à transmettre au moteur.
pub fn set(this: i64, id: &str, prop: &str, value: i64) -> Value {
    check_id(this, id);
    // `${nom}` d'une valeur posée : rempli avec les variables courantes
    // (messages traduits produits par le code).
    let value = with(this, |ui| vars::resolve(&to_json(value), &ui.vars)).unwrap_or(Value::Null);
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

/// `ui.animate(id, nom, options)` : nœud et animation déclarés
/// (`animations:`) ; rend les options (table, ou `null` : celles du `.oc.ui`).
pub fn animate(this: i64, id: &str, name: &str, options: Value) -> Value {
    check_id(this, id);
    let known = with(this, |ui| ui.tree["animations"].get(name).is_some()).unwrap_or(false);
    if !known {
        let names: Vec<String> = with(this, |ui| ui.tree["animations"].as_object().map(|a| a.keys().cloned().collect()).unwrap_or_default()).unwrap_or_default();
        raise(ERR_ANIMATION, format!("animation '{name}' absente du .oc.ui (animations : {})", names.join(", ")));
    }
    if options.is_object() { options } else { Value::Null }
}

/// `ui.stopAnimation(id)` : nœud déclaré.
pub fn stop_animation(this: i64, id: &str) {
    check_id(this, id);
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
    // `OCARA_UI_TRACE=1` : chaque message du moteur sur la sortie d'erreur.
    static TRACE: OnceLock<bool> = OnceLock::new();
    if *TRACE.get_or_init(|| std::env::var_os("OCARA_UI_TRACE").is_some()) {
        eprintln!("UI ← {message}");
    }
    let id = message.get("id").and_then(Value::as_str);
    if let (Some(id), Some(prop)) = (id, message.get("prop").and_then(Value::as_str)) {
        let value = message.get("value").cloned().unwrap_or(Value::Null);
        set_json(this, id, prop, value.clone());
        // Champ marqué `invalid` par une soumission : la saisie le libère.
        if with(this, |ui| ui.state.get(id).and_then(|s| s.get("invalid")) == Some(&Value::Bool(true))).unwrap_or(false) {
            form::set_invalid(this, id, false);
        }
        // Nœud lié : la saisie est écrite dans l'objet, avant l'action suivante.
        if bind::target(this, id, prop).is_some() {
            enqueue(this, Job::Write { id: id.to_string(), prop: prop.to_string(), value });
        }
        return;
    }
    let Some(action) = message.get("action").and_then(Value::as_str) else { return };
    if action == "resize" {
        let class = message.get("value").and_then(Value::as_str).unwrap_or_default().to_string();
        with(this, |ui| ui.size_class = class);
    }
    // Action qui soumet un formulaire incomplet : `onInvalid` à la place.
    if let Some(source) = id {
        let node = props(this, source);
        if form::submits(&node, action) {
            let empty = form::check_submit(this, source);
            if !empty.is_empty() {
                if let Some(invalid) = node.get("onInvalid").and_then(Value::as_str) {
                    let event = Event { action: invalid.to_string(), id: Some(source.to_string()), instance: String::new(), value: Value::from(empty), index: None };
                    enqueue(this, Job::Action(event));
                }
                return;
            }
        }
    }
    // Action propre d'un composant (`composant.nom`) : l'instance d'où elle vient.
    let instance = if action.contains('.') {
        tree(this).map(|t| component::instance_of(&t, action, id)).unwrap_or_default()
    } else {
        String::new()
    };
    let event = Event {
        action: action.to_string(),
        id: id.map(str::to_string),
        instance,
        value: message.get("value").cloned().unwrap_or(Value::Null),
        index: message.get("index").and_then(Value::as_i64),
    };
    enqueue(this, Job::Action(event));
}

/// `ui.refresh()` : relecture des objets liés, sur le thread des actions
/// (jamais pendant qu'un gestionnaire les modifie).
pub fn refresh(this: i64) {
    enqueue(this, Job::Refresh);
}

/// Travail du thread des actions, traité l'un après l'autre.
enum Job {
    Action(Event),
    /// Saisie sur un nœud lié, à écrire dans l'objet.
    Write { id: String, prop: String, value: Value },
    Refresh,
}

static QUEUE: OnceLock<Mutex<mpsc::Sender<(i64, Job)>>> = OnceLock::new();

fn enqueue(this: i64, job: Job) {
    let queue = QUEUE.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(i64, Job)>();
        std::thread::spawn(move || {
            for (this, job) in rx {
                run(this, job);
            }
        });
        Mutex::new(tx)
    });
    let _ = queue.lock().unwrap().send((this, job));
}

fn run(this: i64, job: Job) {
    match job {
        Job::Action(event) => {
            dispatch(this, event);
            if with(this, |ui| !ui.bindings.is_empty()).unwrap_or(false) {
                in_ocara(|| bind::sync(this));
            }
        }
        Job::Write { id, prop, value } => in_ocara(|| bind::write(this, &id, &prop, value)),
        Job::Refresh => in_ocara(|| bind::sync(this)),
    }
}

/// Code Ocara exécuté par le thread des actions.
fn in_ocara(f: impl FnOnce()) {
    unsafe { ffi::__rc_thread_enter() };
    f();
    unsafe { ffi::__rc_thread_leave() };
}

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

#[cfg(test)]
mod tests {
    use serde_json::json;

    /// Arbre affiché selon la classe, puis `id` cherché jusque dans les gabarits de liste.
    #[test]
    fn has_looks_at_the_displayed_variant() {
        let list = |item: serde_json::Value| json!({ "screens": { "home": { "layout": { "type": "column", "children": [{ "type": "list", "id": "cars", "item": item }] } } } });
        let mut tree = list(json!({ "type": "text", "bind": "name" }));
        tree["sizes"] = json!({ "xs": 360, "md": 840, "lg": 1200, "xl": 1600 });
        tree["variants"] = json!({ "lg": list(json!({ "type": "card", "children": [{ "type": "image", "id": "car_photo", "bind": "photo" }] })) });
        let shown = |class: &str| super::declares(&super::displayed(&tree, class)["screens"]["home"]["layout"], "car_photo");
        assert!(!shown(""));
        assert!(!shown("md"));
        assert!(shown("lg"));
        assert!(shown("xl"));
        assert!(super::declares(&tree["screens"]["home"]["layout"], "cars"));
    }

    /// Barre de titre (de tous les écrans, d'un écran) : ses nœuds ont un état.
    #[test]
    fn bar_nodes_are_indexed() {
        let tree = json!({
            "bar": { "type": "input", "id": "top_search" },
            "screens": { "home": {
                "bar": { "type": "button", "id": "home_action" },
                "layout": { "type": "column", "children": [{ "type": "text", "id": "title" }] },
            } },
        });
        let nodes = super::nodes_of(&tree);
        assert!(nodes.contains_key("top_search") && nodes.contains_key("home_action") && nodes.contains_key("title"));
        assert_eq!(super::roots(&tree).len(), 3);
    }

    /// Instance d'une action propre : le plus long `id` d'instance du bon composant.
    #[test]
    fn instance_of_finds_the_innermost_instance() {
        let tree = json!({ "instances": { "best": "carCard", "best.badge": "chip", "other": "carCard" } });
        let of = |action: &str, id: Option<&str>| super::component::instance_of(&tree, action, id);
        assert_eq!(of("carCard.open", Some("best.open")), "best");
        assert_eq!(of("carCard.open", Some("best")), "best");
        assert_eq!(of("chip.remove", Some("best.badge.remove")), "best.badge");
        assert_eq!(of("carCard.open", Some("bestseller.open")), "");
        assert_eq!(of("open_car", Some("best")), "");
        assert_eq!(of("carCard.open", None), "");
    }
}
