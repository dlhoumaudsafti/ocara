/// Raffinement de type (narrowing) d'une variable locale par une condition :
/// `x is T`, `x equal null`, `x not equal null`, combinés par `not`, `and`,
/// `or`. Une union n'étant compatible avec un type que si toutes ses
/// variantes le sont (voir `types_compat`), `if x is null { return }` doit
/// faire de `x:T|null` un `T` pour la suite du bloc.

use crate::parsing::ast::{BinOp, Block, Expr, Literal, Stmt, Type, UnaryOp};

/// Variables raffinées quand la condition est vraie / fausse.
#[derive(Default)]
pub struct Narrowings {
    pub then: Vec<(String, Type)>,
    pub other: Vec<(String, Type)>,
}

/// Raffinements d'une condition ; `declared` rend le type courant d'une
/// variable locale.
pub fn of_condition(cond: &Expr, declared: &dyn Fn(&str) -> Option<Type>) -> Narrowings {
    match cond {
        Expr::IsCheck { expr, ty, .. } => test(expr, ty, declared),
        Expr::Binary { op: BinOp::Equal, left, right, .. } if is_null(right) => test(left, &Type::Null, declared),
        Expr::Binary { op: BinOp::NotEqual, left, right, .. } if is_null(right) => swap(test(left, &Type::Null, declared)),
        Expr::Unary { op: UnaryOp::Not, operand, .. } => swap(of_condition(operand, declared)),
        Expr::Binary { op: BinOp::And, left, right, .. } => {
            let mut n = of_condition(left, declared);
            n.then.extend(of_condition(right, declared).then);
            n.other.clear();
            n
        }
        Expr::Binary { op: BinOp::Or, left, right, .. } => {
            let mut n = of_condition(left, declared);
            n.other.extend(of_condition(right, declared).other);
            n.then.clear();
            n
        }
        _ => Narrowings::default(),
    }
}

fn swap(n: Narrowings) -> Narrowings {
    Narrowings { then: n.other, other: n.then }
}

fn is_null(e: &Expr) -> bool {
    matches!(e, Expr::Literal(Literal::Null, _))
}

fn test(subject: &Expr, tested: &Type, declared: &dyn Fn(&str) -> Option<Type>) -> Narrowings {
    let Expr::Ident(name, _) = subject else { return Narrowings::default() };
    let Some(current) = declared(name) else { return Narrowings::default() };
    let (then, other) = split(&current, tested);
    Narrowings {
        then: then.map(|t| (name.clone(), t)).into_iter().collect(),
        other: other.map(|t| (name.clone(), t)).into_iter().collect(),
    }
}

/// (type si le test réussit, type s'il échoue) — `None` : inchangé.
fn split(current: &Type, tested: &Type) -> (Option<Type>, Option<Type>) {
    let variants: Vec<Type> = match current {
        Type::Union(vs) => vs.clone(),
        Type::Mixed => return (Some(tested.clone()), None),
        other => vec![other.clone()],
    };
    let hit: Vec<Type> = variants.iter().filter(|v| *v == tested).cloned().collect();
    let miss: Vec<Type> = variants.iter().filter(|v| *v != tested).cloned().collect();
    if variants.len() == 1 {
        return (Some(tested.clone()).filter(|t| t != current && is_class(current) && is_class(t)), None);
    }
    let then = if hit.is_empty() { Some(tested.clone()) } else { Some(union_of(hit)) };
    let other = (!miss.is_empty()).then(|| union_of(miss));
    (then, other)
}

fn is_class(t: &Type) -> bool {
    matches!(t, Type::Named(_) | Type::Generic { .. } | Type::Qualified(_))
}

fn union_of(mut types: Vec<Type>) -> Type {
    if types.len() == 1 { types.remove(0) } else { Type::Union(types) }
}

/// Le bloc ne se termine jamais normalement (`return`, `raise`, `break`,
/// `continue`, `result`, ou `if` dont toutes les branches divergent).
pub fn diverges(block: &Block) -> bool {
    block.stmts.last().is_some_and(stmt_diverges)
}

fn stmt_diverges(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return { .. } | Stmt::Raise { .. } | Stmt::Break { .. } | Stmt::Continue { .. } | Stmt::Result { .. } => true,
        Stmt::If { then_block, elseif, else_block: Some(else_block), .. } =>
            diverges(then_block) && elseif.iter().all(|(_, b)| diverges(b)) && diverges(else_block),
        _ => false,
    }
}
