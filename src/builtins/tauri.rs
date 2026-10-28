// ─────────────────────────────────────────────────────────────────────────────
// ocara.Tauri — classe builtin pour interface Tauri (UI desktop)
//
// Méthodes d'instance :
//   use Tauri(options: map<string, mixed>) → Tauri
//   ui.listen(event: string, callback: function)
//   ui.emit(event: string, data: mixed)
//   ui.dialog(options: map<string, mixed>) → string
//   ui.alert(title, text) / ui.confirm(title, text) → bool
//   ui.pickFile(extensions: array<string>) → string / ui.saveFile(name) → string
//   ui.notify(title, text)
//   ui.setMenu(items) / ui.setContextMenu(items) / ui.setDevTools(enabled) / ui.openDevTools()
//   ui.setAlwaysOnTop / setDecorations / setResizable(enabled)
//   ui.setTray(options) / ui.removeTray() / ui.addShortcut(shortcut, action) / ui.removeShortcut(shortcut)
//
// Convention runtime : Tauri_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

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

pub fn tauri_class() -> ClassInfo {
    let mut methods = HashMap::new();

    // Constructeur (use Tauri)
    methods.insert(
        "__init__".to_string(),
        static_m(vec![ ("options", Type::Map(Box::new(Type::String), Box::new(Type::Mixed))) ], Type::Named("Tauri".to_string())),
    );

    // listen
    methods.insert(
        "listen".to_string(),
        inst_m(vec![ ("event", Type::String), ("callback", Type::Function { ret_ty: Box::new(Type::Void), param_tys: vec![Type::String] }) ], Type::Void),
    );

    // emit
    methods.insert(
        "emit".to_string(),
        inst_m(vec![ ("event", Type::String), ("data", Type::Mixed) ], Type::Void),
    );

    // dialog
    methods.insert(
        "dialog".to_string(),
        inst_m(vec![ ("options", Type::Map(Box::new(Type::String), Box::new(Type::Mixed))) ], Type::String),
    );

    // Boîtes de dialogue et notifications natives (noms standard d'ocara.UI)
    methods.insert("alert".to_string(), inst_m(vec![ ("title", Type::String), ("text", Type::String) ], Type::Void));
    methods.insert("confirm".to_string(), inst_m(vec![ ("title", Type::String), ("text", Type::String) ], Type::Bool));
    methods.insert("pickFile".to_string(), inst_m(vec![ ("extensions", Type::Array(Box::new(Type::String))) ], Type::String));
    methods.insert("saveFile".to_string(), inst_m(vec![ ("name", Type::String) ], Type::String));
    methods.insert("notify".to_string(), inst_m(vec![ ("title", Type::String), ("text", Type::String) ], Type::Void));

    // Menus natifs : entrées `map<string, mixed>` (text, action, shortcut,
    // enabled, checked, separator, items), clic → ui.listen(action, …)
    let menu_items = || Type::Array(Box::new(Type::Map(Box::new(Type::String), Box::new(Type::Mixed))));
    methods.insert("setMenu".to_string(), inst_m(vec![ ("items", menu_items()) ], Type::Void));
    methods.insert("setContextMenu".to_string(), inst_m(vec![ ("items", menu_items()) ], Type::Void));
    methods.insert("setDevTools".to_string(), inst_m(vec![ ("enabled", Type::Bool) ], Type::Void));
    methods.insert("openDevTools".to_string(), inst_m(vec![], Type::Void));

    // Options de fenêtre (aussi clés de `use Tauri` : alwaysOnTop, decorations, resizable…)
    methods.insert("setAlwaysOnTop".to_string(), inst_m(vec![ ("enabled", Type::Bool) ], Type::Void));
    methods.insert("setDecorations".to_string(), inst_m(vec![ ("enabled", Type::Bool) ], Type::Void));
    methods.insert("setResizable".to_string(), inst_m(vec![ ("enabled", Type::Bool) ], Type::Void));

    // Icône de zone de notification et raccourcis globaux (clic/appui → ui.listen(action, …))
    methods.insert("setTray".to_string(), inst_m(vec![ ("options", Type::Map(Box::new(Type::String), Box::new(Type::Mixed))) ], Type::Void));
    methods.insert("removeTray".to_string(), inst_m(vec![], Type::Void));
    methods.insert("addShortcut".to_string(), inst_m(vec![ ("shortcut", Type::String), ("action", Type::String) ], Type::Void));
    methods.insert("removeShortcut".to_string(), inst_m(vec![ ("shortcut", Type::String) ], Type::Void));

    // Getters/setters et gestion fenêtre (parité runtime)
    methods.insert("getTitle".to_string(), inst_m(vec![], Type::String));
    methods.insert("setTitle".to_string(), inst_m(vec![ ("title", Type::String) ], Type::Void));
    methods.insert("getWidth".to_string(), inst_m(vec![], Type::Int));
    methods.insert("setWidth".to_string(), inst_m(vec![ ("width", Type::Int) ], Type::Void));
    methods.insert("getHeight".to_string(), inst_m(vec![], Type::Int));
    methods.insert("setHeight".to_string(), inst_m(vec![ ("height", Type::Int) ], Type::Void));
    methods.insert("getUrl".to_string(), inst_m(vec![], Type::String));
    methods.insert("setUrl".to_string(), inst_m(vec![ ("url", Type::String) ], Type::Void));
    methods.insert("open".to_string(), inst_m(vec![], Type::Void));
    methods.insert("close".to_string(), inst_m(vec![], Type::Void));
    methods.insert("isOpen".to_string(), inst_m(vec![], Type::Bool));
    methods.insert("focus".to_string(), inst_m(vec![], Type::Void));
    methods.insert("minimize".to_string(), inst_m(vec![], Type::Void));
    methods.insert("maximize".to_string(), inst_m(vec![], Type::Void));
    methods.insert("restore".to_string(), inst_m(vec![], Type::Void));
    methods.insert("hasFocus".to_string(), inst_m(vec![], Type::Bool));
    methods.insert("isMinimized".to_string(), inst_m(vec![], Type::Bool));
    methods.insert("isMaximized".to_string(), inst_m(vec![], Type::Bool));

    // run — lance réellement la fenêtre native (bloque jusqu'à sa fermeture).
    // Doit être appelée en dernier : tout enregistrement (listen, etc.) doit
    // se faire avant, le crate tauri prend possession du thread appelant.
    methods.insert("run".to_string(), inst_m(vec![], Type::Void));

    // handler(name, method) — expose une méthode Ocara comme commande IPC JS.
    // Le second paramètre est une référence de méthode statique (ex:
    // HomeController::updateDateTime, self::maMethode) : sa vraie signature
    // est vérifiée statiquement par le lowering (voir tauri_handler.rs), pas
    // ici — Mixed sert seulement à faire accepter la syntaxe par la sema.
    methods.insert("handler".to_string(), inst_m(
        vec![("name", Type::String), ("method", Type::Mixed)],
        Type::Void,
    ));

    // handlers({"nom": Classe::methode, ...}) — enregistrement en masse, désucré
    // à la compilation vers plusieurs handler(nom, methode) (voir tauri_handler.rs,
    // try_lower_tauri_handlers_call) : même mécanisme, même registre anti-doublon.
    // Mixed pour la même raison que ci-dessus : la vraie validation (littéral map,
    // clés string, valeurs StaticConst) est faite au lowering, pas ici.
    methods.insert("handlers".to_string(), inst_m(
        vec![("map", Type::Mixed)],
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
