//! `UIComponent` (`ui.component(nom)`) : composant déclaré sous `components:`
//! d'un `.oc.ui` (docs/oc-ui.md, « Composants »). Valeur native comptée qui
//! garde le pointeur de son `UI` et le nom du composant : ses actions propres
//! (`self.nom` dans le `layout`, `<composant>.nom` dans l'arbre) se lient par
//! `card.on(nom, gestionnaire)`, ses instances (`id` → composant) viennent de
//! l'arbre (`instances`). Si l'objet `UI` a été libéré, les méthodes ne
//! trouvent plus son état : `on` lève `UIException`, les autres rendent vide.

use serde_json::Value;

use crate::ffi::{__native_data, __native_new, from_json, raise, str_arg, string};

pub const ERR_COMPONENT: i64 = 106;

struct Component {
    ui: i64,
    name: String,
}

extern "C" fn drop_component(data: i64) {
    drop(unsafe { Box::from_raw(data as *mut Component) });
}

fn with<R>(val: i64, f: impl FnOnce(&Component) -> R, default: R) -> R {
    let data = unsafe { __native_data(val, drop_component as extern "C" fn(i64) as usize as i64) };
    if data == 0 { default } else { f(unsafe { &*(data as *const Component) }) }
}

/// Composants déclarés et leurs actions propres, toutes variantes réunies.
fn declared(ui: i64) -> Option<serde_json::Map<String, Value>> {
    crate::tree(ui).and_then(|t| t.get("components").and_then(Value::as_object).cloned())
}

/// `ui.component(nom)` → `UIComponent` ; `UIException` 106 si le composant
/// n'est pas déclaré.
#[unsafe(no_mangle)]
pub extern "C" fn UI_component(this: i64, name: i64) -> i64 {
    let name = str_arg(name);
    let components = declared(this).unwrap_or_default();
    if !components.contains_key(&name) {
        let known: Vec<&str> = components.keys().map(String::as_str).collect();
        raise(ERR_COMPONENT, format!("composant '{name}' absent du .oc.ui (composants : {})", known.join(", ")));
    }
    let component = Component { ui: this, name };
    unsafe { __native_new(Box::into_raw(Box::new(component)) as i64, drop_component as extern "C" fn(i64) as usize as i64) }
}

/// `card.on(action, gestionnaire)` : action propre `self.action` du composant.
#[unsafe(no_mangle)]
pub extern "C" fn UIComponent_on(component: i64, action: i64, handler: i64) {
    let action = str_arg(action);
    let Some((ui, name)) = with(component, |c| Some((c.ui, c.name.clone())), None) else { return };
    let own = declared(ui).and_then(|c| c.get(&name).cloned()).unwrap_or(Value::Null);
    if !own.as_array().is_some_and(|a| a.iter().any(|v| v == &Value::String(action.clone()))) {
        raise(crate::ERR_ACTION, format!("composant '{name}' : action propre 'self.{action}' absente (actions : {own})"));
    }
    crate::on(ui, &format!("{name}.{action}"), handler);
}

/// `card.name()` → `string`.
#[unsafe(no_mangle)]
pub extern "C" fn UIComponent_name(component: i64) -> i64 {
    with(component, |c| string(&c.name), 0)
}

/// `card.instances()` → `array<string>` : `id` des instances posées avec un `id`.
#[unsafe(no_mangle)]
pub extern "C" fn UIComponent_instances(component: i64) -> i64 {
    let ids = with(component, |c| {
        let instances = crate::tree(c.ui).and_then(|t| t.get("instances").and_then(Value::as_object).cloned()).unwrap_or_default();
        instances.iter().filter(|(_, comp)| comp.as_str() == Some(c.name.as_str())).map(|(id, _)| Value::String(id.clone())).collect()
    }, Vec::new());
    from_json(&Value::Array(ids))
}

/// `card.id(instance, interne)` → `instance.interne` (`interne` vide : l'instance).
#[unsafe(no_mangle)]
pub extern "C" fn UIComponent_id(_component: i64, instance: i64, inner: i64) -> i64 {
    let (instance, inner) = (str_arg(instance), str_arg(inner));
    string(&if inner.is_empty() { instance } else { format!("{instance}.{inner}") })
}

/// Instance d'où vient l'action propre `action` (`composant.nom`) déclenchée
/// par le nœud `id` : l'instance du composant dont l'`id` est le plus long
/// préfixe de `id` (instances imbriquées comprises), `""` sinon.
pub fn instance_of(tree: &Value, action: &str, id: Option<&str>) -> String {
    let (Some((component, _)), Some(id)) = (action.split_once('.'), id) else { return String::new() };
    let Some(instances) = tree.get("instances").and_then(Value::as_object) else { return String::new() };
    instances.iter()
        .filter(|(inst, comp)| comp.as_str() == Some(component) && (id == inst.as_str() || id.starts_with(&format!("{inst}."))))
        .map(|(inst, _)| inst.clone())
        .max_by_key(String::len)
        .unwrap_or_default()
}
