//! `self` emprunté sans retenue : une méthode est appelée avec un receveur
//! que l'appelant atteint par un chemin de références qu'il tient. Seul un
//! appel (fonction, méthode, constructeur, builtin), l'attente d'une tâche
//! ou une écriture dans un champ d'un AUTRE objet ou dans un conteneur peut
//! couper ce chemin et libérer `self` pendant la méthode ; écrire dans les
//! champs de `self` lui-même ne le peut pas. Un corps qui n'en contient
//! aucun se passe de la retenue/relâche de `self` (accesseurs, `init`
//! simples).
//!
//! Un appel ne coupe rien quand l'appelée ne peut rien libérer de ce que
//! tient un autre qu'elle :
//! - `self.m(…)` : `m` se résout dans le programme (pas chez un parent
//!   builtin) et TOUTES ses définitions, de n'importe quelle classe (liaison
//!   dynamique), sont sûres (`self_safe`) ;
//! - `x.m(…)` sur une locale ou un paramètre déclaré d'une classe du
//!   programme, `f(…)` sur une fonction du programme : l'appelée est pure —
//!   aucune écriture de conteneur ni de champ compté (même de son `self` ;
//!   un champ `int`/`float`/`bool` ne libère rien), et seulement des appels
//!   purs ;
//! - un builtin sans rappel ni effet sur ses arguments (`Math`, `String`,
//!   conversions scalaires de `Convert`).
//!
//! Points fixes calculés une fois par programme (`analyze`).

use std::collections::{HashMap, HashSet};

use crate::parsing::ast::{Block, ClassDecl, ClassMember, Param, Program, Type};

use super::borrow_scan::{assigned_names, Scan};

/// Résultat de `analyze`, par classe : méthodes que `self.m(…)` appelle
/// sans risquer de libérer `self` (`safe`), méthodes pures appelables sur un
/// receveur de cette classe (`pure`) ; fonctions pures du programme.
#[derive(Default)]
pub struct Borrowing {
    pub safe: HashMap<String, HashSet<String>>,
    pub pure: HashMap<String, HashSet<String>>,
    pub pure_functions: HashSet<String>,
    pub program_types: HashSet<String>,
    /// Par classe : champs `int`/`float`/`bool`, hérités compris.
    pub scalar_fields: HashMap<String, HashSet<String>>,
}

/// Appels qu'un corps peut faire sans libérer ce que tient son appelant.
pub struct Calls<'a> {
    /// Classe de `self`, pour une méthode.
    pub self_class: Option<&'a str>,
    /// Champ non compté de la classe : `scalar_field(c, f)`.
    pub scalar_field: &'a dyn Fn(&str, &str) -> bool,
    /// `self.m(…)`.
    pub self_call: &'a dyn Fn(&str) -> bool,
    /// `x.m(…)`, `x` de classe `c` : `pure_call(c, m)`.
    pub pure_call: &'a dyn Fn(&str, &str) -> bool,
    /// `f(…)`.
    pub pure_fn: &'a dyn Fn(&str) -> bool,
    /// `C::m(…)` : vrai seulement pour un builtin, faux si `C` est du programme.
    pub user_type: &'a dyn Fn(&str) -> bool,
}

/// Vrai si rien dans `body` ne peut libérer `self`.
pub fn self_stays_alive(params: &[Param], body: &Block, calls: &Calls) -> bool {
    Scan::new(params, body, calls, false).block(body)
}

/// Vrai si `body` ne peut rien libérer de ce que tient un autre que lui.
fn is_pure(params: &[Param], body: &Block, calls: &Calls) -> bool {
    Scan::new(params, body, calls, true).block(body)
}

/// Paramètres que le corps peut emprunter sans les retenir, comme `self` :
/// ceux d'un corps pur, ni réaffectés, ni capturés (aucun `nameless` ni
/// `try` — leurs cellules ne comptent que les locales comptées), ni
/// variadiques ni à valeur par défaut (que l'appelant ne tient pas
/// forcément). `self` relève de `self_stays_alive`.
pub fn borrowed_params(params: &[Param], body: &Block, calls: &Calls) -> HashSet<String> {
    let mut scan = Scan::new(params, body, calls, true);
    scan.closed = true;
    if !scan.block(body) {
        return HashSet::new();
    }
    let mut assigned = HashSet::new();
    assigned_names(body, &mut assigned);
    params.iter()
        .filter(|p| p.name != "self" && !p.is_variadic && p.default_value.is_none() && !assigned.contains(&p.name))
        .map(|p| p.name.clone())
        .collect()
}

pub fn analyze(program: &Program) -> Borrowing {
    let classes: HashMap<&str, &ClassDecl> = program.classes.iter().map(|c| (c.name.as_str(), c)).collect();
    let modules: HashMap<&str, &[ClassMember]> = program.modules.iter().map(|m| (m.name.as_str(), m.members.as_slice())).collect();
    // `m` sur une instance de `class` se résout-il dans le programme ?
    let resolves = |class: &str, method: &str| -> bool {
        let mut current = classes.get(class).copied();
        while let Some(c) = current {
            if own_methods(c, &modules).iter().any(|(name, _, _)| *name == method) {
                return true;
            }
            current = c.extends.as_deref().and_then(|p| classes.get(p).copied());
        }
        false
    };
    let program_types: HashSet<String> = program.classes.iter().map(|c| c.name.clone())
        .chain(program.modules.iter().map(|m| m.name.clone()))
        .chain(program.interfaces.iter().map(|i| i.name.clone()))
        .chain(program.enums.iter().map(|e| e.name.clone()))
        .chain(program.generics.iter().map(|g| g.name.clone()))
        .collect();
    let user_type = |name: &str| program_types.contains(name);
    let scalar_fields: HashMap<String, HashSet<String>> = program.classes.iter().map(|c| {
        let mut fields = HashSet::new();
        let mut current = Some(c);
        while let Some(c) = current {
            let members = c.members.iter()
                .chain(c.modules.iter().filter_map(|m| modules.get(m.as_str()).copied()).flat_map(|members| members.iter()));
            for m in members {
                if let ClassMember::Field { name, ty: Type::Int | Type::Float | Type::Bool, .. } = m {
                    fields.insert(name.clone());
                }
            }
            current = c.extends.as_deref().and_then(|p| classes.get(p).copied());
        }
        (c.name.clone(), fields)
    }).collect();
    let scalar_field = |c: &str, f: &str| scalar_fields.get(c).is_some_and(|fs| fs.contains(f));
    let definitions: Vec<(&str, &str, &[Param], &Block)> = program.classes.iter()
        .flat_map(|c| own_methods(c, &modules).into_iter().map(move |(name, params, body)| (c.name.as_str(), name, params, body)))
        .collect();
    let names: HashSet<&str> = definitions.iter().map(|(_, name, _, _)| *name).collect();

    // Méthodes (par nom, toutes définitions) et fonctions pures.
    let mut pure: HashSet<&str> = HashSet::new();
    let mut pure_fns: HashSet<&str> = HashSet::new();
    loop {
        let mut grown = false;
        let pure_call = |c: &str, m: &str| resolves(c, m) && pure.contains(m);
        let pure_fn = |f: &str| pure_fns.contains(f);
        let mut new_methods = Vec::new();
        for &name in names.iter().filter(|n| !pure.contains(*n)) {
            let all = definitions.iter().filter(|(_, n, _, _)| *n == name).all(|(class, _, params, body)| {
                let self_call = |m: &str| pure_call(class, m);
                is_pure(params, body, &Calls { self_class: Some(class), scalar_field: &scalar_field, self_call: &self_call, pure_call: &pure_call, pure_fn: &pure_fn, user_type: &user_type })
            });
            if all {
                new_methods.push(name);
            }
        }
        let mut new_fns = Vec::new();
        for f in program.functions.iter().filter(|f| !pure_fns.contains(f.name.as_str())) {
            let self_call = |_: &str| false;
            if is_pure(&f.params, &f.body, &Calls { self_class: None, scalar_field: &scalar_field, self_call: &self_call, pure_call: &pure_call, pure_fn: &pure_fn, user_type: &user_type }) {
                new_fns.push(f.name.as_str());
            }
        }
        grown |= !new_methods.is_empty() || !new_fns.is_empty();
        pure.extend(new_methods);
        pure_fns.extend(new_fns);
        if !grown {
            break;
        }
    }

    // Méthodes sûres pour `self.m(…)` : les pures, et celles qui n'écrivent
    // que dans leur `self`.
    let pure_call = |c: &str, m: &str| resolves(c, m) && pure.contains(m);
    let pure_fn = |f: &str| pure_fns.contains(f);
    let mut safe: HashSet<&str> = pure.clone();
    loop {
        let mut grown = false;
        for &name in names.iter() {
            if safe.contains(name) {
                continue;
            }
            let all_safe = definitions.iter().filter(|(_, n, _, _)| *n == name).all(|(class, _, params, body)| {
                let self_call = |m: &str| resolves(class, m) && safe.contains(m);
                self_stays_alive(params, body, &Calls { self_class: Some(class), scalar_field: &scalar_field, self_call: &self_call, pure_call: &pure_call, pure_fn: &pure_fn, user_type: &user_type })
            });
            if all_safe {
                safe.insert(name);
                grown = true;
            }
        }
        if !grown {
            break;
        }
    }
    let per_class = |set: &HashSet<&str>| program.classes.iter().map(|c| {
        let methods = set.iter().filter(|m| resolves(&c.name, m)).map(|m| m.to_string()).collect();
        (c.name.clone(), methods)
    }).collect();
    Borrowing {
        safe: per_class(&safe),
        pure: per_class(&pure),
        pure_functions: pure_fns.into_iter().map(str::to_string).collect(),
        program_types,
        scalar_fields,
    }
}

/// Méthodes d'instance d'une classe, celles de ses `modules` comprises.
fn own_methods<'p>(class: &'p ClassDecl, modules: &HashMap<&str, &'p [ClassMember]>) -> Vec<(&'p str, &'p [Param], &'p Block)> {
    class.members.iter()
        .chain(class.modules.iter().filter_map(|m| modules.get(m.as_str()).copied()).flat_map(|members| members.iter()))
        .filter_map(|m| match m {
            ClassMember::Method { decl, is_static: false, .. } => Some((decl.name.as_str(), decl.params.as_slice(), &decl.body)),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::{lexer::Lexer, parser::Parser};

    fn analyzed(src: &str) -> Borrowing {
        let tokens = Lexer::new(src).tokenize().expect("lex ok");
        analyze(&Parser::new(tokens).parse_program().expect("parse ok"))
    }

    fn calls(src: &str) -> HashMap<String, HashSet<String>> {
        analyzed(src).safe
    }

    #[test]
    fn a_pure_helper_is_a_safe_call_but_a_builtin_parent_method_is_not() {
        let c = calls("class Counter {
    public property total:int = 0
    public method add(n:int): void { self.total += self.weight(n) }
    public method weight(n:int): int { return n * 2 }
    public method log(): void { IO::writeln(\"x\") }
    public method addLogged(n:int): void { self.log() }
}
class Srv extends HTTPServer {
    public method start(): void { self.run() }
}
");
        let counter = &c["Counter"];
        assert!(counter.contains("weight") && counter.contains("add"), "{:?}", counter);
        assert!(!counter.contains("log") && !counter.contains("addLogged"), "{:?}", counter);
        assert!(!c["Srv"].contains("start") && !c["Srv"].contains("run"));
    }

    #[test]
    fn an_unsafe_override_makes_the_call_unsafe_everywhere() {
        let c = calls("class Base {
    public method step(): int { return 1 }
    public method go(): int { return self.step() }
}
class Child extends Base {
    public method step(): int {
        IO::writeln(\"y\")
        return 2
    }
}
");
        assert!(!c["Base"].contains("step") && !c["Base"].contains("go"), "{:?}", c["Base"]);
    }

    #[test]
    fn pure_calls_on_typed_receivers_and_builtins() {
        let b = analyzed("class Pt {
    public property x:int = 0
    public property tag:string = \"\"
    public method dist(n:int): int { return Math::abs(n - self.x) }
    public method bump(): void { self.x += 1 }
    public method rename(): void { self.tag = \"y\" }
}
class Acc {
    public property total:int = 0
    public method add(p:Pt, n:int): void {
        self.total += p.dist(n)
        p.bump()
    }
    public method relabel(p:Pt): void { p.rename() }
    public method untyped(p:mixed): void { p.bump() }
}
function twice(n:int): int { return n * 2 }
function shout(): void { IO::writeln(\"x\") }
");
        assert!(b.pure["Pt"].contains("dist") && b.pure["Pt"].contains("bump"), "{:?}", b.pure["Pt"]);
        assert!(!b.pure["Pt"].contains("rename"), "champ compté : {:?}", b.pure["Pt"]);
        assert!(b.safe["Pt"].contains("rename"));
        assert!(b.pure["Acc"].contains("add") && !b.pure["Acc"].contains("relabel") && !b.pure["Acc"].contains("untyped"));
        assert!(b.pure_functions.contains("twice") && !b.pure_functions.contains("shout"));
    }

    #[test]
    fn a_program_class_named_like_a_builtin_is_not_pure() {
        let b = analyzed("class Math {
    public static method abs(n:int): int {
        IO::writeln(\"x\")
        return n
    }
}
class Pt {
    public property x:int = 0
    public method dist(n:int): int { return Math::abs(n - self.x) }
}
");
        assert!(!b.pure["Pt"].contains("dist") && !b.safe["Pt"].contains("dist"));
    }

    #[test]
    fn borrowed_params_exclude_reassigned_and_captured_ones() {
        let src = "class Pt {
    public property x:int = 0
}
function keep(p:Pt, q:Pt): Pt {
    q = p
    return q
}
function cap(p:Pt): void {
    var f:Function<int()> = nameless(): int { return p.x }
}
";
        let tokens = Lexer::new(src).tokenize().expect("lex ok");
        let program = Parser::new(tokens).parse_program().expect("parse ok");
        let no = |_: &str| false;
        let no2 = |_: &str, _: &str| false;
        let calls = Calls { self_class: None, scalar_field: &no2, self_call: &no, pure_call: &no2, pure_fn: &no, user_type: &no };
        let keep = &program.functions[0];
        let borrowed = borrowed_params(&keep.params, &keep.body, &calls);
        assert!(borrowed.contains("p") && !borrowed.contains("q"), "{:?}", borrowed);
        let cap = &program.functions[1];
        assert!(borrowed_params(&cap.params, &cap.body, &calls).is_empty());
    }
}
