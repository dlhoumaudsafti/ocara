// ─────────────────────────────────────────────────────────────────────────────
// ocara.IO — classe builtin statique
//
// Méthodes statiques :
//   IO::write(val)        → void     affiche val sans saut de ligne final
//   IO::writeln(val)      → void     affiche val suivi d'un saut de ligne
//   IO::read()            → string   lit une ligne (sans le \n final)
//   IO::readln()          → string   alias de read()
//   IO::read_int()        → int      lit une ligne et la convertit en int
//   IO::read_float()      → float    lit une ligne et la convertit en float
//   IO::read_bool()       → bool     lit une ligne ("true"/"1" → true, sinon false)
//   IO::read_array(sep)   → string[] lit une ligne et la découpe selon sep
//   IO::read_map(sep,kv)  → map<string,string>
//                                    lit une ligne, découpe selon sep, puis
//                                    chaque partie selon kv (ex. "=" pour "k=v")
//
// Convention runtime : IO_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::ast::Type;
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
    }
}

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // IO::write(val) → void
    methods.insert("write".into(), m(
        vec![("val", Type::Mixed)],
        Type::Void,
    ));

    // IO::writeln(val) → void
    methods.insert("writeln".into(), m(
        vec![("val", Type::Mixed)],
        Type::Void,
    ));

    // IO::read() → string
    methods.insert("read".into(), m(
        vec![],
        Type::String,
    ));

    // IO::readln() → string  (alias de read)
    methods.insert("readln".into(), m(
        vec![],
        Type::String,
    ));

    // IO::read_int() → int
    methods.insert("read_int".into(), m(
        vec![],
        Type::Int,
    ));

    // IO::read_float() → float
    methods.insert("read_float".into(), m(
        vec![],
        Type::Float,
    ));

    // IO::read_bool() → bool
    methods.insert("read_bool".into(), m(
        vec![],
        Type::Bool,
    ));

    // IO::read_array(sep) → string[]
    methods.insert("read_array".into(), m(
        vec![("sep", Type::String)],
        Type::Array(Box::new(Type::String)),
    ));

    // IO::read_map(sep, kv) → map<string, string>
    methods.insert("read_map".into(), m(
        vec![("sep", Type::String), ("kv", Type::String)],
        Type::Map(Box::new(Type::String), Box::new(Type::String)),
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
