// ─────────────────────────────────────────────────────────────────────────────
// ocara.System — classe builtin statique
//
// Constantes de classe :
//   System::OS    → string  "linux" | "macos" | "windows"
//   System::ARCH  → string  "x86_64" | "aarch64" | …
//
// Méthodes statiques :
//   System::exec(cmd)              → string   exécute cmd, retourne stdout
//   System::passthrough(cmd)       → int      exécute cmd (stdio hérité), retourne le code
//   System::execCode(cmd)         → int      exécute cmd, retourne uniquement le code de sortie
//   System::exit(code)             → void     quitte le processus
//   System::env(name)              → string   lit une variable d'environnement ("" si absente)
//   System::setEnv(name, value)   → void     définit une variable d'environnement
//   System::cwd()                  → string   répertoire de travail courant
//   System::sleep(ms)              → void     pause en millisecondes
//   System::pid()                  → int      PID du processus courant
//   System::args()                 → string[] arguments de la ligne de commande
//
// Convention runtime : System_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::ast::{Type, Visibility};
use crate::sema::symbols::{ClassInfo, FuncSig};

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

    // System::exec(cmd) → string
    methods.insert("exec".into(), m(
        vec![("cmd", Type::String)],
        Type::String,
    ));

    // System::passthrough(cmd) → int
    methods.insert("passthrough".into(), m(
        vec![("cmd", Type::String)],
        Type::Int,
    ));

    // System::execCode(cmd) → int
    methods.insert("execCode".into(), m(
        vec![("cmd", Type::String)],
        Type::Int,
    ));

    // System::exit(code) → void
    methods.insert("exit".into(), m(
        vec![("code", Type::Int)],
        Type::Void,
    ));

    // System::env(name) → string
    methods.insert("env".into(), m(
        vec![("name", Type::String)],
        Type::String,
    ));

    // System::setEnv(name, value) → void
    methods.insert("setEnv".into(), m(
        vec![("name", Type::String), ("value", Type::String)],
        Type::Void,
    ));

    // System::cwd() → string
    methods.insert("cwd".into(), m(
        vec![],
        Type::String,
    ));

    // System::sleep(ms) → void
    methods.insert("sleep".into(), m(
        vec![("ms", Type::Int)],
        Type::Void,
    ));

    // System::pid() → int
    methods.insert("pid".into(), m(
        vec![],
        Type::Int,
    ));

    // System::args() → string[]
    methods.insert("args".into(), m(
        vec![],
        Type::Array(Box::new(Type::String)),
    ));

    // Constantes de classe
    let mut class_consts: HashMap<String, (Type, Visibility)> = HashMap::new();
    class_consts.insert("OS".into(),   (Type::String, Visibility::Public));
    class_consts.insert("ARCH".into(), (Type::String, Visibility::Public));

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts,
        is_opaque:    false,
    }
}
