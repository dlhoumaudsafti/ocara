// ─────────────────────────────────────────────────────────────────────────────
// ocara.UI — interface native décrite par un `.oc.ui` (docs/oc-ui.md,
// docs/builtins/UI.md). Desktop : fenêtre Tauri et moteur de rendu
// (runtime_tauri/src/ui) ; Android : moteur Compose (runtime_android/ui).
//
//   use UI("x.oc.ui", vars)           // lu et validé à la compilation (src/core/ui_file.rs)
//   ui.setVars(vars) / ui.sizeClass() → string / ui.has(id) → bool
//   ui.on(action, Classe::methode)    // gestionnaire (ui:UI, event:UIEvent): void
//   ui.get(id, prop) → mixed / ui.set(id, prop, valeur) / ui.setItems(id, rows)
//   ui.navigate(screen) / ui.back() / ui.run() / ui.close()
//   ui.toast(text) / ui.alert(title, text) / ui.confirm(title, text) → bool
//   ui.pickFile(extensions) → string / ui.saveFile(name) → string / ui.notify(title, text)
//   ui.tauri() → Tauri (desktop ; UIException 104 sur Android)
//   ui.postMessage(id, data) / ui.eval(id, script) → string / ui.webBack(id) / webForward(id) / reload(id)
//
//   ui.component(name) → UIComponent  // composant déclaré sous `components:`
//
//   event.action() / event.id() / event.instance() → string, event.value() → mixed, event.index() → int
//
//   card.on(action, Classe::methode) / card.name() → string
//   card.instances() → array<string> / card.id(instance, interne) → string
//
// Convention runtime : UI_<méthode>, UIEvent_<méthode>, UIComponent_<méthode>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

fn inst_m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
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
    FuncSig { is_static: true, ..inst_m(params, ret_ty) }
}

fn class_of(methods: HashMap<String, FuncSig>) -> ClassInfo {
    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}

/// Gestionnaire d'action : `(ui:UI, event:UIEvent): void`.
fn handler_ty() -> Type {
    Type::Function {
        ret_ty: Box::new(Type::Void),
        param_tys: vec![Type::Named("UI".into()), Type::Named("UIEvent".into())],
    }
}

pub fn class() -> ClassInfo {
    let mut methods = HashMap::new();
    // Constructeur : `use UI("x.oc.ui", vars)` ; le chemin devient l'arbre JSON,
    // `vars` (facultatif : complété par src/core/ui_file.rs) remplit les `${nom}`.
    methods.insert("__init__".to_string(), static_m(vec![("path", Type::String), ("vars", Type::Map(Box::new(Type::String), Box::new(Type::Mixed)))], Type::Named("UI".into())));
    // Textes : nouvelles valeurs des variables `${nom}` (changement de langue).
    methods.insert("setVars".to_string(), inst_m(vec![("vars", Type::Map(Box::new(Type::String), Box::new(Type::Mixed)))], Type::Void));
    // Classe de taille de la fenêtre ou de l'écran (`xxs` … `xxl`, "" avant run()).
    methods.insert("sizeClass".to_string(), inst_m(vec![], Type::String));
    // Nœud `id` présent dans la variante affichée (gabarits de liste compris) :
    // ne calculer une donnée que si l'interface l'affiche.
    methods.insert("has".to_string(), inst_m(vec![("id", Type::String)], Type::Bool));
    // Composant déclaré (`components:`) : ses actions propres (`self.nom`), ses instances.
    methods.insert("component".to_string(), inst_m(vec![("name", Type::String)], Type::Named("UIComponent".into())));
    methods.insert("on".to_string(), inst_m(vec![("action", Type::String), ("handler", handler_ty())], Type::Void));
    methods.insert("get".to_string(), inst_m(vec![("id", Type::String), ("prop", Type::String)], Type::Mixed));
    methods.insert("set".to_string(), inst_m(vec![("id", Type::String), ("prop", Type::String), ("value", Type::Mixed)], Type::Void));
    methods.insert("setItems".to_string(), inst_m(vec![("id", Type::String), ("rows", Type::Mixed)], Type::Void));
    // Liaisons (`bind: user.name` hors d'une liste) : l'objet d'une classe
    // relié à la racine ; accesseur généré par la sema (src/sema/ui_bind.rs).
    methods.insert("bind".to_string(), inst_m(vec![("root", Type::String), ("object", Type::Mixed)], Type::Void));
    methods.insert("unbind".to_string(), inst_m(vec![("root", Type::String)], Type::Void));
    methods.insert("refresh".to_string(), inst_m(vec![], Type::Void));
    methods.insert("navigate".to_string(), inst_m(vec![("screen", Type::String)], Type::Void));
    methods.insert("back".to_string(), inst_m(vec![], Type::Void));
    methods.insert("run".to_string(), inst_m(vec![], Type::Void));
    methods.insert("close".to_string(), inst_m(vec![], Type::Void));
    methods.insert("toast".to_string(), inst_m(vec![("text", Type::String)], Type::Void));
    methods.insert("alert".to_string(), inst_m(vec![("title", Type::String), ("text", Type::String)], Type::Void));
    methods.insert("confirm".to_string(), inst_m(vec![("title", Type::String), ("text", Type::String)], Type::Bool));
    methods.insert("pickFile".to_string(), inst_m(vec![("extensions", Type::Array(Box::new(Type::String)))], Type::String));
    methods.insert("saveFile".to_string(), inst_m(vec![("name", Type::String)], Type::String));
    // Spécificités desktop : la fenêtre Tauri de l'interface (`import ocara.Tauri`).
    methods.insert("tauri".to_string(), inst_m(vec![], Type::Named("Tauri".into())));
    // Composant `webview`
    methods.insert("postMessage".to_string(), inst_m(vec![("id", Type::String), ("data", Type::Mixed)], Type::Void));
    methods.insert("eval".to_string(), inst_m(vec![("id", Type::String), ("script", Type::String)], Type::String));
    methods.insert("webBack".to_string(), inst_m(vec![("id", Type::String)], Type::Void));
    methods.insert("webForward".to_string(), inst_m(vec![("id", Type::String)], Type::Void));
    methods.insert("reload".to_string(), inst_m(vec![("id", Type::String)], Type::Void));
    methods.insert("notify".to_string(), inst_m(vec![("title", Type::String), ("text", Type::String)], Type::Void));
    class_of(methods)
}

pub fn event_class() -> ClassInfo {
    let mut methods = HashMap::new();
    methods.insert("action".to_string(), inst_m(vec![], Type::String));
    methods.insert("id".to_string(), inst_m(vec![], Type::String));
    methods.insert("value".to_string(), inst_m(vec![], Type::Mixed));
    methods.insert("index".to_string(), inst_m(vec![], Type::Int));
    // `id` de l'instance du composant dont vient l'action propre, "" sinon.
    methods.insert("instance".to_string(), inst_m(vec![], Type::String));
    class_of(methods)
}

/// `UIComponent` (`ui.component(nom)`) : valeur native comptée, liée à son `UI`.
pub fn component_class() -> ClassInfo {
    let mut methods = HashMap::new();
    // Action propre `self.action` du composant → gestionnaire (ui:UI, event:UIEvent): void.
    methods.insert("on".to_string(), inst_m(vec![("action", Type::String), ("handler", handler_ty())], Type::Void));
    methods.insert("name".to_string(), inst_m(vec![], Type::String));
    // `id` des instances posées avec un `id` (toutes variantes de taille).
    methods.insert("instances".to_string(), inst_m(vec![], Type::Array(Box::new(Type::String))));
    // `id` d'un nœud interne d'une instance : `instance.interne` (`interne` "" : l'instance).
    methods.insert("id".to_string(), inst_m(vec![("instance", Type::String), ("inner", Type::String)], Type::String));
    class_of(methods)
}
