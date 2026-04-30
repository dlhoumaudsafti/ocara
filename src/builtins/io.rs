// ─────────────────────────────────────────────────────────────────────────────
// ocara.IO — classe builtin statique
//
// Méthodes statiques :
//   IO::write(val)        → void     affiche val sans saut de ligne final
//   IO::writeln(val)      → void     affiche val suivi d'un saut de ligne
//   IO::read()            → string   lit une ligne (sans le \n final)
//   IO::readln()          → string   alias de read()
//   IO::readInt()        → int      lit une ligne et la convertit en int
//   IO::readFloat()      → float    lit une ligne et la convertit en float
//   IO::readBool()       → bool     lit une ligne ("true"/"1" → true, sinon false)
//   IO::readArray(sep)   → string[] lit une ligne et la découpe selon sep
//   IO::readMap(sep,kv)  → map<string,string>
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
        required_params_count: len,
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

    // IO::readInt() → int
    methods.insert("readInt".into(), m(
        vec![],
        Type::Int,
    ));

    // IO::readFloat() → float
    methods.insert("readFloat".into(), m(
        vec![],
        Type::Float,
    ));

    // IO::readBool() → bool
    methods.insert("readBool".into(), m(
        vec![],
        Type::Bool,
    ));

    // IO::readArray(sep) → string[]
    methods.insert("readArray".into(), m(
        vec![("sep", Type::String)],
        Type::Array(Box::new(Type::String)),
    ));

    // IO::readMap(sep, kv) → map<string, string>
    methods.insert("readMap".into(), m(
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
