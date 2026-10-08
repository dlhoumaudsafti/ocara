//! Génération du comptage de références — voir docs/roadmap.d/memoire-refcount.md.
//!
//! - Une valeur produite par un appel, `use`, un littéral de conteneur, une
//!   concaténation, un gabarit ou une closure est possédée (+1) : enregistrée
//!   comme temporaire du statement courant, relâchée à sa fin sauf si un
//!   stockage la récupère (`take`).
//! - Une lecture (variable, champ, index) est empruntée : un stockage la
//!   retient.
//! - Les locales comptées sont relâchées en sortie de portée (fin de bloc,
//!   `return`, `break`, `continue`), jamais une variable capturée par une
//!   closure (sa cellule n'est pas encore comptée : fuite sûre).

use crate::ir::inst::{Inst, Value};
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;
use crate::parsing::ast::{BinOp, Expr, Type};

/// Builtins dont le résultat est un élément emprunté au conteneur.
const BORROWING_BUILTINS: &[&str] = &["Array_get", "Array_first", "Array_last", "Map_get"];

/// Builtins de conteneur dont le résultat `mixed` est un élément : compté
/// seulement si le type d'élément du receveur l'est.
const ELEMENT_BUILTINS: &[&str] = &["Array_pop"];

pub fn counted(builder: &LowerBuilder, ty: &Type) -> bool {
    crate::lower::builder::rc_layout::is_counted(ty, &builder.module.rc_objects)
}

fn emit_call(builder: &mut LowerBuilder, func: &str, v: &Value) {
    builder.emit(Inst::Call { dest: None, func: func.into(), args: vec![v.clone()], ret_ty: IrType::Void });
}

pub fn retain(builder: &mut LowerBuilder, v: &Value) {
    emit_call(builder, "__rc_retain", v);
}

pub fn release(builder: &mut LowerBuilder, v: &Value) {
    if !builder.rc_no_release {
        emit_call(builder, "__rc_release", v);
    }
}

/// `array<int|float|bool>`/`map<K, int|float|bool>` : éléments bruts.
pub fn mark_raw_if_primitive(builder: &mut LowerBuilder, ty: &Type, v: &Value) {
    let elem = match ty {
        Type::Array(inner) | Type::Map(_, inner) => inner.as_ref(),
        _ => return,
    };
    if matches!(elem, Type::Int | Type::Float | Type::Bool) {
        let dest = builder.new_value();
        builder.emit(Inst::Call { dest: Some(dest), func: "__rc_mark_raw".into(), args: vec![v.clone()], ret_ty: IrType::Ptr });
    }
}

// ── Temporaires ─────────────────────────────────────────────────────────────

pub fn begin_temps(builder: &mut LowerBuilder) {
    builder.rc_temps.push(Vec::new());
}

/// Fin du statement : relâche ses temporaires (si le chemin courant continue).
pub fn end_temps(builder: &mut LowerBuilder) {
    let frame = builder.rc_temps.pop().unwrap_or_default();
    if !builder.is_terminated() {
        for v in frame.iter().rev() {
            release(builder, v);
        }
    }
}

/// Relâche tout de suite les temporaires du statement courant (condition
/// d'un `if`/`while`, évaluée dans un bloc qui ne domine pas la suite).
pub fn flush_temps(builder: &mut LowerBuilder) {
    let frame = builder.rc_temps.last_mut().map(std::mem::take).unwrap_or_default();
    for v in frame.iter().rev() {
        release(builder, v);
    }
}

pub fn track(builder: &mut LowerBuilder, v: &Value) {
    if let Some(frame) = builder.rc_temps.last_mut() {
        frame.push(v.clone());
    }
}

/// Retire `v` des temporaires : vrai s'il était possédé.
pub fn claim(builder: &mut LowerBuilder, v: &Value) -> bool {
    for frame in builder.rc_temps.iter_mut().rev() {
        if let Some(i) = frame.iter().position(|t| t == v) {
            frame.remove(i);
            return true;
        }
    }
    false
}

/// Un stockage compté prend `v` : la référence d'un temporaire possédé est
/// transférée, une valeur empruntée est retenue.
pub fn take(builder: &mut LowerBuilder, v: &Value) {
    if !claim(builder, v) {
        retain(builder, v);
    }
}

/// Stockage hors des locales (env d'une tâche, valeur par défaut d'une
/// closure, frame d'un générateur) : jamais relâché ensuite.
pub fn keep(builder: &mut LowerBuilder, v: &Value, is_counted: bool) {
    if !claim(builder, v) && is_counted {
        retain(builder, v);
    }
}

/// Arguments rangés dans l'env d'une tâche `async` : ils doivent survivre à
/// l'appelant (jamais relâchés ensuite). `self` éventuel en tête.
pub fn keep_task_args(builder: &mut LowerBuilder, callee: &str, vals: &[Value]) {
    let declared = builder.fn_param_types.get(callee).cloned().unwrap_or_default();
    let types: Vec<IrType> = if declared.len() + 1 == vals.len() {
        std::iter::once(IrType::Ptr).chain(declared).collect()
    } else {
        declared
    };
    for (i, v) in vals.iter().enumerate() {
        let is_counted = types.len() == vals.len() && types[i] == IrType::Ptr;
        keep(builder, v, is_counted);
    }
}

/// Relâche (sans les retirer) les temporaires des statements ouverts à
/// partir de la profondeur `depth` — sortie anticipée.
pub fn release_temps_from(builder: &mut LowerBuilder, depth: usize) {
    let pending: Vec<Value> = builder.rc_temps.iter().skip(depth).rev().flat_map(|f| f.iter().rev().cloned()).collect();
    for v in &pending {
        release(builder, v);
    }
}

// ── Locales ─────────────────────────────────────────────────────────────────

pub fn begin_scope(builder: &mut LowerBuilder) {
    builder.rc_scopes.push(Vec::new());
}

pub fn end_scope(builder: &mut LowerBuilder) {
    let frame = builder.rc_scopes.pop().unwrap_or_default();
    if !builder.is_terminated() {
        let names: Vec<String> = frame.into_iter().rev().collect();
        release_locals(builder, &names);
    }
}

/// Relâche les locales `names`, dans cet ordre.
fn release_locals(builder: &mut LowerBuilder, names: &[String]) {
    for name in names {
        if builder.heap_promoted.contains(name) || builder.captured_vars.contains_key(name) {
            continue;
        }
        if let Some((v, _)) = builder.load_local(name) {
            release(builder, &v);
        }
    }
}

/// `consumed` comptée : relâchée juste après le statement de son premier
/// usage (voir `release_consumed_used_in`).
pub fn declare_consumed(builder: &mut LowerBuilder, name: &str) {
    builder.rc_consumed.insert(name.to_string(), builder.loop_depth);
}

/// Après `stmt` : relâche les `consumed` comptées qu'il lit. Un usage dans
/// une boucle plus profonde que la déclaration est répété : la fin de
/// portée s'en charge. Un usage qui a conservé la valeur l'a retenue.
pub fn release_consumed_used_in(builder: &mut LowerBuilder, stmt: &crate::parsing::ast::Stmt) {
    let live = builder.rc_consumed.clone();
    let used = crate::lower::stmt::ownership::consumed_reads(stmt, &|name| live.contains_key(name));
    for name in used {
        if live.get(&name).is_some_and(|depth| builder.loop_depth > *depth) {
            continue;
        }
        builder.rc_consumed.remove(&name);
        let Some(frame) = builder.rc_scopes.iter_mut().rev().find(|f| f.contains(&name)) else { continue };
        frame.retain(|n| *n != name);
        if let Some((v, _)) = builder.load_local(&name) {
            release(builder, &v);
        }
    }
}

/// Locale comptée : relâchée à la fin de la portée courante.
pub fn declare(builder: &mut LowerBuilder, name: &str) {
    builder.rc_counted_locals.insert(name.to_string());
    if let Some(frame) = builder.rc_scopes.last_mut() {
        frame.push(name.to_string());
    }
}

pub fn is_counted_local(builder: &LowerBuilder, name: &str) -> bool {
    builder.rc_counted_locals.contains(name)
        || builder.captured_vars.get(name).is_some_and(|(_, _, ty)| *ty == IrType::Ptr)
}

/// Relâche les locales des portées ouvertes à partir de `depth`.
pub fn release_scopes_from(builder: &mut LowerBuilder, depth: usize) {
    let names: Vec<String> = builder.rc_scopes.iter().skip(depth).rev().flat_map(|f| f.iter().rev().cloned()).collect();
    release_locals(builder, &names);
}

/// Sortie de la fonction : temporaires et locales de toutes les portées.
pub fn release_all(builder: &mut LowerBuilder) {
    release_temps_from(builder, 0);
    release_scopes_from(builder, 0);
}

/// Affectation d'une locale comptée : nouvelle valeur prise, ancienne relâchée.
pub fn assign_local(builder: &mut LowerBuilder, name: &str, val: Value, fresh: bool) {
    let old = builder.load_local(name).map(|(v, _)| v);
    if !fresh {
        take(builder, &val);
    }
    builder.store_local(name, val);
    if let Some(old) = old {
        release(builder, &old);
    }
}

// ── Fonctions ───────────────────────────────────────────────────────────────

/// Portée de la fonction : chaque paramètre compté (emprunté à l'appelant)
/// est retenu, puis relâché à la sortie comme une locale.
pub fn begin_function(builder: &mut LowerBuilder, params: &[crate::parsing::ast::Param]) {
    builder.block_scope_stack.push(Vec::new());
    begin_scope(builder);
    for param in params {
        let ty = if param.is_variadic { Type::Array(Box::new(param.ty.clone())) } else { param.ty.clone() };
        if !counted(builder, &ty) {
            continue;
        }
        if let Some((v, _)) = builder.load_local(&param.name) {
            retain(builder, &v);
            declare(builder, &param.name);
        }
    }
}

pub fn end_function(builder: &mut LowerBuilder) {
    end_scope(builder);
    builder.block_scope_stack.pop();
}

// ── Boucles ─────────────────────────────────────────────────────────────────

/// Entrée dans une boucle : `break`/`continue` relâcheront les temporaires
/// des statements ouverts dans son corps.
pub fn enter_loop(builder: &mut LowerBuilder) {
    builder.rc_loop_temps.push(builder.rc_temps.len());
}

pub fn exit_loop(builder: &mut LowerBuilder) {
    builder.rc_loop_temps.pop();
}

/// `break`/`continue` : temporaires du corps et locales des portées
/// ouvertes depuis `scope_depth`.
pub fn release_loop_exit(builder: &mut LowerBuilder, scope_depth: usize) {
    let temps_depth = builder.rc_loop_temps.last().copied().unwrap_or(builder.rc_temps.len());
    release_temps_from(builder, temps_depth);
    release_scopes_from(builder, scope_depth);
}

/// Portée d'une itération, qui porte les variables de boucle.
pub fn begin_iteration(builder: &mut LowerBuilder) {
    builder.block_scope_stack.push(Vec::new());
    begin_scope(builder);
}

pub fn end_iteration(builder: &mut LowerBuilder) {
    end_scope(builder);
    builder.block_scope_stack.pop();
}

/// Variable de boucle liée à un élément emprunté : retenue, relâchée en fin
/// d'itération, si son type est compté.
pub fn bind_loop_var(builder: &mut LowerBuilder, name: &str, ty: Option<&Type>, val: &Value) {
    if ty.is_some_and(|ty| counted(builder, ty)) {
        retain(builder, val);
        declare(builder, name);
    } else {
        builder.rc_counted_locals.remove(name);
    }
}

// ── Possession d'une expression ─────────────────────────────────────────────

/// Vrai si `expr` produit une valeur comptée possédée (+1). Les littéraux de
/// conteneur s'enregistrent eux-mêmes (`lower_array_literal`).
pub fn produces_owned(builder: &LowerBuilder, expr: &Expr) -> bool {
    match expr {
        Expr::Call { .. } | Expr::StaticCall { .. } => call_owned(builder, expr),
        Expr::New { class, .. } => builder.module.rc_objects.contains(class),
        Expr::Template { .. } | Expr::Nameless { .. } | Expr::Range { .. } => true,
        Expr::Binary { op: BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod, .. } => {
            crate::lower::expr::expr_ir_type_pub(builder, expr) == IrType::Ptr
        }
        Expr::Ident(name, _) => {
            !builder.locals.contains_key(name.as_str())
                && !builder.captured_vars.contains_key(name.as_str())
                && !builder.frame_vars.contains_key(name.as_str())
                && builder.fn_param_types.contains_key(name.as_str())
        }
        _ => false,
    }
}

fn call_owned(builder: &LowerBuilder, expr: &Expr) -> bool {
    let Some(key) = crate::lower::expr::helpers::call_key(builder, expr) else { return false };
    if builder.async_funcs.contains(&key) || is_async_call(builder, expr) {
        return false;
    }
    if BORROWING_BUILTINS.contains(&key.as_str()) {
        return false;
    }
    let Some(ty) = builder.module.call_ret_types.get(&key) else { return false };
    if !counted(builder, ty) {
        return false;
    }
    if ELEMENT_BUILTINS.contains(&key.as_str()) {
        return receiver_elem_counted(builder, expr);
    }
    true
}

fn is_async_call(builder: &LowerBuilder, expr: &Expr) -> bool {
    match expr {
        Expr::Call { callee, .. } => matches!(callee.as_ref(), Expr::Ident(n, _) if builder.async_funcs.contains(n)),
        _ => false,
    }
}

fn receiver_elem_counted(builder: &LowerBuilder, expr: &Expr) -> bool {
    let receiver = match expr {
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Field { object, .. } => object.as_ref(),
            _ => return false,
        },
        Expr::StaticCall { args, .. } => match args.first() {
            Some(a) => a,
            None => return false,
        },
        _ => return false,
    };
    crate::lower::expr::helpers::elem_type_after_index(builder, receiver)
        .is_some_and(|elem| counted(builder, &elem))
}
