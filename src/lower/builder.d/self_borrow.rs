//! `self` emprunté sans retenue : une méthode est appelée avec un receveur
//! que l'appelant atteint par un chemin de références qu'il tient. Seul un
//! appel (fonction, méthode, constructeur, builtin), l'attente d'une tâche
//! ou une écriture dans un champ d'un AUTRE objet ou dans un conteneur peut
//! couper ce chemin et libérer `self` pendant la méthode ; écrire dans les
//! champs de `self` lui-même ne le peut pas. Un corps qui n'en contient
//! aucun se passe de la retenue/relâche de `self` (accesseurs, `init`
//! simples).
//!
//! Un appel de méthode sur `self` (`self.poids(n)`) est sûr quand la méthode
//! appelée ne peut pas libérer son propre `self` : elle se résout dans le
//! programme (pas chez un parent builtin) et TOUTES ses définitions, de
//! n'importe quelle classe (liaison dynamique), sont elles-mêmes sûres —
//! point fixe calculé une fois par programme (`safe_self_calls`).

use std::collections::{HashMap, HashSet};

use crate::parsing::ast::{Block, ClassDecl, ClassMember, Expr, Program, Stmt, TemplatePartExpr};

/// Par classe : méthodes que `self.m(…)` appelle sans risquer de libérer `self`.
pub type SafeSelfCalls = HashMap<String, HashSet<String>>;

/// Vrai si rien dans `body` ne peut libérer `self` ; `safe_call(m)` : un
/// appel `self.m(…)` en est incapable.
pub fn self_stays_alive(body: &Block, safe_call: &dyn Fn(&str) -> bool) -> bool {
    body.stmts.iter().all(|s| stmt_safe(s, safe_call))
}

pub fn safe_self_calls(program: &Program) -> SafeSelfCalls {
    let classes: HashMap<&str, &ClassDecl> = program.classes.iter().map(|c| (c.name.as_str(), c)).collect();
    let modules: HashMap<&str, &[ClassMember]> = program.modules.iter().map(|m| (m.name.as_str(), m.members.as_slice())).collect();
    // `self.m()` dans `class` se résout-il dans le programme ?
    let resolves = |class: &str, method: &str| -> bool {
        let mut current = classes.get(class).copied();
        while let Some(c) = current {
            if own_methods(c, &modules).iter().any(|(name, _)| *name == method) {
                return true;
            }
            current = c.extends.as_deref().and_then(|p| classes.get(p).copied());
        }
        false
    };
    let definitions: Vec<(&str, &str, &Block)> = program.classes.iter()
        .flat_map(|c| own_methods(c, &modules).into_iter().map(move |(name, body)| (c.name.as_str(), name, body)))
        .collect();
    let mut safe: HashSet<String> = HashSet::new();
    loop {
        let mut grown = false;
        let names: HashSet<&str> = definitions.iter().map(|(_, name, _)| *name).collect();
        for name in names {
            if safe.contains(name) {
                continue;
            }
            let all_safe = definitions.iter().filter(|(_, n, _)| *n == name).all(|(class, _, body)| {
                self_stays_alive(body, &|m| resolves(class, m) && safe.contains(m))
            });
            if all_safe {
                safe.insert(name.to_string());
                grown = true;
            }
        }
        if !grown {
            break;
        }
    }
    program.classes.iter().map(|c| {
        let calls = safe.iter().filter(|m| resolves(&c.name, m)).cloned().collect();
        (c.name.clone(), calls)
    }).collect()
}

/// Méthodes d'instance d'une classe, celles de ses `modules` comprises.
fn own_methods<'p>(class: &'p ClassDecl, modules: &HashMap<&str, &'p [ClassMember]>) -> Vec<(&'p str, &'p Block)> {
    class.members.iter()
        .chain(class.modules.iter().filter_map(|m| modules.get(m.as_str()).copied()).flat_map(|members| members.iter()))
        .filter_map(|m| match m {
            ClassMember::Method { decl, is_static: false, .. } => Some((decl.name.as_str(), &decl.body)),
            _ => None,
        })
        .collect()
}

fn stmt_safe(stmt: &Stmt, safe_call: &dyn Fn(&str) -> bool) -> bool {
    let block = |b: &Block| self_stays_alive(b, safe_call);
    let expr = |e: &Expr| expr_safe(e, safe_call);
    match stmt {
        Stmt::Var { value, .. } | Stmt::Const { value, .. } | Stmt::Expr(value) | Stmt::Raise { value, .. } => expr(value),
        Stmt::Return { value, .. } | Stmt::Result { value, .. } => value.as_ref().is_none_or(expr),
        Stmt::Break { .. } | Stmt::Continue { .. } => true,
        Stmt::If { condition, then_block, elseif, else_block, .. } =>
            expr(condition) && block(then_block)
                && elseif.iter().all(|(c, b)| expr(c) && block(b))
                && else_block.as_ref().is_none_or(block),
        Stmt::While { condition, body, .. } => expr(condition) && block(body),
        Stmt::ForIn { iter, body, .. } | Stmt::ForMap { iter, body, .. } => expr(iter) && block(body),
        Stmt::Switch { subject, cases, default, .. } =>
            expr(subject) && cases.iter().all(|c| block(&c.body)) && default.as_ref().is_none_or(block),
        Stmt::Try { body, handlers, .. } => block(body) && handlers.iter().all(|h| block(&h.body)),
        Stmt::Assign { target, value, .. } => expr(value) && match target {
            Expr::Ident(..) => true,
            Expr::Field { object, .. } => matches!(object.as_ref(), Expr::SelfExpr(_)),
            _ => false,
        },
        _ => false,
    }
}

fn expr_safe(expr: &Expr, safe_call: &dyn Fn(&str) -> bool) -> bool {
    let safe = |e: &Expr| expr_safe(e, safe_call);
    match expr {
        Expr::Call { callee, args, .. } => match callee.as_ref() {
            Expr::Field { object, field, .. } if matches!(object.as_ref(), Expr::SelfExpr(_)) =>
                safe_call(field) && args.iter().all(safe),
            _ => false,
        },
        Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_)
        | Expr::StaticConst { .. } | Expr::Nameless { .. } => true,
        Expr::Field { object, .. } => safe(object),
        Expr::Binary { left, right, .. } => safe(left) && safe(right),
        Expr::Unary { operand, .. } => safe(operand),
        Expr::IsCheck { expr, .. } => safe(expr),
        Expr::IncDec { target, .. } => safe(target),
        Expr::Index { object, index, .. } => safe(object) && safe(index),
        Expr::Range { start, end, .. } => safe(start) && safe(end),
        Expr::Array { elements, .. } => elements.iter().all(safe),
        Expr::Map { entries, .. } => entries.iter().all(|(k, v)| safe(k) && safe(v)),
        Expr::Template { parts, .. } => parts.iter().all(|p| match p {
            TemplatePartExpr::Expr(e) => safe(e),
            _ => true,
        }),
        Expr::Match { subject, arms, .. } => safe(subject) && arms.iter().all(|a| safe(&a.body)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::{lexer::Lexer, parser::Parser};

    fn calls(src: &str) -> SafeSelfCalls {
        let tokens = Lexer::new(src).tokenize().expect("lex ok");
        safe_self_calls(&Parser::new(tokens).parse_program().expect("parse ok"))
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
}
