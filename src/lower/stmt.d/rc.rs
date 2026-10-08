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
    emit_call(builder, "__rc_release", v);
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
    builder.rc_temps_emit.push(false);
}

/// Un `emit` suspend le générateur : les temporaires des statements ouverts
/// ne seront plus valides à la reprise.
pub fn mark_emit(builder: &mut LowerBuilder) {
    builder.rc_temps_emit.iter_mut().for_each(|crossed| *crossed = true);
}

fn crossed_emit(builder: &LowerBuilder, depth: usize) -> bool {
    builder.rc_temps_emit.get(depth).copied().unwrap_or(false)
}

/// Fin du statement : relâche ses temporaires (si le chemin courant continue).
pub fn end_temps(builder: &mut LowerBuilder) {
    let crossed = builder.rc_temps_emit.pop().unwrap_or(false);
    let frame = builder.rc_temps.pop().unwrap_or_default();
    if !builder.is_terminated() && !crossed {
        for v in frame.iter().rev() {
            release_temp(builder, v);
        }
    }
}

/// Relâche un temporaire et remet son mot de déroulement à zéro.
fn release_temp(builder: &mut LowerBuilder, v: &Value) {
    release(builder, v);
    clear_temp_word(builder, v);
}

fn clear_temp_word(builder: &mut LowerBuilder, v: &Value) {
    let Some(word) = builder.rc_temp_words.get(v).cloned() else { return };
    let zero = builder.new_value();
    builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
    builder.emit(Inst::Store { ptr: word, src: zero });
}

/// Relâche tout de suite les temporaires du statement courant (condition
/// d'un `if`/`while`, évaluée dans un bloc qui ne domine pas la suite).
pub fn flush_temps(builder: &mut LowerBuilder) {
    if builder.rc_temps_emit.last().copied().unwrap_or(false) {
        return;
    }
    let frame = builder.rc_temps.last_mut().map(std::mem::take).unwrap_or_default();
    for v in frame.iter().rev() {
        release_temp(builder, v);
    }
}

/// Temporaire possédé du statement courant ; rangé dans un mot de
/// déroulement pour qu'un `raise` traversant le rende.
pub fn track(builder: &mut LowerBuilder, v: &Value) {
    let Some(frame) = builder.rc_temps.last_mut() else { return };
    frame.push(v.clone());
    let Some(base) = builder.rc_unwind_base.clone() else { return };
    let word = new_word(builder, &base);
    builder.emit(Inst::Store { ptr: word.clone(), src: v.clone() });
    builder.rc_temp_words.insert(v.clone(), word);
}

/// Retire `v` des temporaires : vrai s'il était possédé.
pub fn claim(builder: &mut LowerBuilder, v: &Value) -> bool {
    let Some(frame) = builder.rc_temps.iter_mut().rev().find(|f| f.contains(v)) else { return false };
    frame.retain(|t| t != v);
    clear_temp_word(builder, v);
    true
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
    let pending: Vec<Value> = builder.rc_temps.iter().enumerate().skip(depth).rev()
        .filter(|(i, _)| !crossed_emit(builder, *i))
        .flat_map(|(_, f)| f.iter().rev().cloned())
        .collect();
    for v in &pending {
        release_temp(builder, v);
    }
}

// ── Locales ─────────────────────────────────────────────────────────────────

/// Locale d'une portée ouverte. `word` : son mot dans le tableau de
/// déroulement de la fonction (voir `begin_unwind`), remis à zéro quand la
/// locale est rendue.
#[derive(Clone)]
pub struct RcLocal {
    pub name: String,
    pub counted: bool,
    pub word: Option<Value>,
}

pub fn begin_scope(builder: &mut LowerBuilder) {
    builder.rc_scopes.push(Vec::new());
}

pub fn end_scope(builder: &mut LowerBuilder) {
    let frame = builder.rc_scopes.pop().unwrap_or_default();
    if !builder.is_terminated() {
        let locals: Vec<RcLocal> = frame.into_iter().rev().collect();
        release_locals(builder, &locals);
    }
}

/// Relâche les locales `locals`, dans cet ordre : une locale promue rend sa
/// cellule (qui relâche la valeur quand plus aucune closure ne la tient),
/// une locale comptée sa valeur.
fn release_locals(builder: &mut LowerBuilder, locals: &[RcLocal]) {
    for local in locals {
        release_local(builder, local);
    }
}

fn release_local(builder: &mut LowerBuilder, local: &RcLocal) {
    if builder.captured_vars.contains_key(&local.name) {
        return;
    }
    if builder.heap_promoted.contains(&local.name) {
        if let Some((cell, _, _)) = builder.locals.get(&local.name).cloned() {
            release(builder, &cell);
        }
    } else if local.counted {
        let Some((v, _)) = builder.load_local(&local.name) else { return };
        release(builder, &v);
        if builder.rc_generator {
            clear_local(builder, &local.name);
        }
    } else {
        return;
    }
    if let Some(word) = &local.word {
        let zero = builder.new_value();
        builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
        builder.emit(Inst::Store { ptr: word.clone(), src: zero });
    }
}

/// Champ du frame d'un générateur remis à zéro une fois relâché : la
/// destruction du frame (`__drop`) ne le relâchera pas une seconde fois.
fn clear_local(builder: &mut LowerBuilder, name: &str) {
    let zero = builder.new_value();
    builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
    builder.store_local(name, zero);
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
        let Some(frame) = builder.rc_scopes.iter_mut().rev().find(|f| f.iter().any(|l| l.name == name)) else { continue };
        let Some(i) = frame.iter().position(|l| l.name == name) else { continue };
        let local = frame.remove(i);
        release_local(builder, &local);
    }
}

/// Locale comptée : relâchée à la fin de la portée courante.
pub fn declare(builder: &mut LowerBuilder, name: &str) {
    builder.rc_counted_locals.insert(name.to_string());
    let word = bind_word(builder, name);
    if let Some(frame) = builder.rc_scopes.last_mut() {
        frame.push(RcLocal { name: name.to_string(), counted: true, word });
    }
}

/// Locale non comptée : seule sa cellule, si elle est promue, est rendue.
pub fn declare_plain(builder: &mut LowerBuilder, name: &str) {
    builder.rc_counted_locals.remove(name);
    if let Some(frame) = builder.rc_scopes.last_mut() {
        frame.push(RcLocal { name: name.to_string(), counted: false, word: None });
    }
}

pub fn is_counted_local(builder: &LowerBuilder, name: &str) -> bool {
    builder.rc_counted_locals.contains(name)
        || builder.captured_vars.get(name).is_some_and(|(_, _, ty)| *ty == IrType::Ptr)
}

/// Relâche les locales des portées ouvertes à partir de `depth`.
pub fn release_scopes_from(builder: &mut LowerBuilder, depth: usize) {
    let locals: Vec<RcLocal> = builder.rc_scopes.iter().skip(depth).rev().flat_map(|f| f.iter().rev().cloned()).collect();
    release_locals(builder, &locals);
}

/// Sortie de la fonction : ressources fermées d'abord (leur objet doit
/// encore être vivant), puis temporaires et locales de toutes les portées.
pub fn release_all(builder: &mut LowerBuilder) {
    crate::lower::stmt::ownership::emit_early_exit_drops(builder, 0);
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

// ── Déroulement par `raise` ─────────────────────────────────────────────────

/// Les locales comptées de la fonction vivront dans un tableau de mots sur
/// sa pile, enregistré auprès du runtime (`__rc_unwind_push`) : un `raise`
/// qui traverse la fonction les rend avant son `longjmp`. Le tableau n'est
/// matérialisé qu'à la fin (`finish_unwind`), s'il sert.
pub fn begin_unwind(builder: &mut LowerBuilder) {
    builder.rc_unwind_base = Some(builder.new_value());
    builder.rc_unwind_words = 0;
}

/// Déplace la locale `name` dans un nouveau mot du tableau de déroulement.
fn bind_word(builder: &mut LowerBuilder, name: &str) -> Option<Value> {
    let base = builder.rc_unwind_base.clone()?;
    if builder.frame_vars.contains_key(name) || builder.heap_promoted.contains(name) {
        return None;
    }
    let (slot, ty, mutable) = builder.locals.get(name).cloned()?;
    let word = new_word(builder, &base);
    let cur = builder.new_value();
    builder.emit(Inst::Load { dest: cur.clone(), ptr: slot, ty: ty.clone() });
    builder.emit(Inst::Store { ptr: word.clone(), src: cur });
    builder.locals.insert(name.to_string(), (word.clone(), ty, mutable));
    Some(word)
}

fn new_word(builder: &mut LowerBuilder, base: &Value) -> Value {
    let offset = builder.new_value();
    builder.emit(Inst::ConstInt { dest: offset.clone(), value: builder.rc_unwind_words as i64 * 8 });
    builder.rc_unwind_words += 1;
    let word = builder.new_value();
    builder.emit(Inst::Add { dest: word.clone(), lhs: base.clone(), rhs: offset, ty: IrType::I64 });
    word
}

/// Mot de déroulement d'une locale promue : il porte désormais la cellule.
fn word_for_cell(builder: &mut LowerBuilder, name: &str, cell: &Value) {
    let Some(base) = builder.rc_unwind_base.clone() else { return };
    let Some(pos) = builder.rc_scopes.iter().rposition(|f| f.iter().any(|l| l.name == name)) else { return };
    let existing = builder.rc_scopes[pos].iter().rev().find(|l| l.name == name).and_then(|l| l.word.clone());
    let word = match existing {
        Some(word) => word,
        None => new_word(builder, &base),
    };
    builder.emit(Inst::Store { ptr: word.clone(), src: cell.clone() });
    if let Some(local) = builder.rc_scopes[pos].iter_mut().rev().find(|l| l.name == name) {
        local.word = Some(word);
    }
}

/// Matérialise le tableau de déroulement s'il sert : alloué et enregistré
/// en tête de fonction, retiré avant chaque `Return`.
pub fn finish_unwind(builder: &mut LowerBuilder) {
    let Some(base) = builder.rc_unwind_base.take() else { return };
    let words = builder.rc_unwind_words;
    if words == 0 {
        return;
    }
    let count = builder.new_value();
    let prologue = vec![
        Inst::AllocaWords { dest: base.clone(), words },
        Inst::ConstInt { dest: count.clone(), value: words as i64 },
        Inst::Call { dest: None, func: "__rc_unwind_push".into(), args: vec![base, count], ret_ty: IrType::Void },
    ];
    for block in builder.func.blocks.iter_mut() {
        let mut insts = Vec::with_capacity(block.insts.len() + 1);
        for inst in block.insts.drain(..) {
            if matches!(inst, Inst::Return { .. }) {
                insts.push(Inst::Call { dest: None, func: "__rc_unwind_pop".into(), args: vec![], ret_ty: IrType::Void });
            }
            insts.push(inst);
        }
        block.insts = insts;
    }
    if let Some(entry) = builder.func.blocks.first_mut() {
        entry.insts.splice(0..0, prologue);
    }
}

// ── Cellules de capture ─────────────────────────────────────────────────────

/// Promeut la locale `name` (slot `slot`) dans une cellule comptée partagée
/// avec les closures/`try` qui la capturent : la référence du slot passe à
/// la cellule (`self`, emprunté, y est retenu). La cellule est rendue en fin
/// de portée de la locale (`release_locals`).
pub fn promote_to_cell(builder: &mut LowerBuilder, name: &str, slot: Value, ty: &IrType) -> Value {
    let counted = builder.rc_counted_locals.contains(name) || name == "self";
    let flag = builder.new_value();
    builder.emit(Inst::ConstInt { dest: flag.clone(), value: counted as i64 });
    let cell = builder.new_value();
    builder.emit(Inst::Call { dest: Some(cell.clone()), func: "__alloc_locked_cell".into(), args: vec![flag], ret_ty: IrType::Ptr });
    let cur = builder.new_value();
    builder.emit(Inst::Load { dest: cur.clone(), ptr: slot, ty: ty.clone() });
    if name == "self" {
        retain(builder, &cur);
    }
    builder.emit(Inst::Call { dest: None, func: "__locked_cell_set".into(), args: vec![cell.clone(), cur], ret_ty: IrType::Void });
    word_for_cell(builder, name, &cell);
    cell
}

// ── Fonctions ───────────────────────────────────────────────────────────────

/// Portée de la fonction : chaque paramètre compté (emprunté à l'appelant)
/// est retenu, puis relâché à la sortie comme une locale.
pub fn begin_function(builder: &mut LowerBuilder, params: &[crate::parsing::ast::Param]) {
    builder.block_scope_stack.push(Vec::new());
    begin_scope(builder);
    if builder.locals.contains_key("self") {
        declare_plain(builder, "self");
    }
    for param in params {
        let ty = if param.is_variadic { Type::Array(Box::new(param.ty.clone())) } else { param.ty.clone() };
        if !counted(builder, &ty) {
            declare_plain(builder, &param.name);
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
    crate::lower::stmt::ownership::emit_early_exit_drops(builder, scope_depth);
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
        declare_plain(builder, name);
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
        Expr::Resolve { expr: task, .. } => resolved_type(builder, task).is_some_and(|ty| counted(builder, &ty)),
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

/// Type `T` produit par `resolve task` : variable `Resolvable<T>` ou appel
/// direct d'une fonction `async` qui retourne `T`.
fn resolved_type(builder: &LowerBuilder, task: &Expr) -> Option<Type> {
    match task {
        Expr::Ident(name, _) => builder.resolvable_types.get(name).cloned(),
        _ => builder.module.call_ret_types.get(&crate::lower::expr::helpers::call_key(builder, task)?).cloned(),
    }
}

fn call_owned(builder: &LowerBuilder, expr: &Expr) -> bool {
    if let Expr::Call { callee, .. } = expr {
        if let Expr::Ident(name, _) = callee.as_ref() {
            if builder.func_vars.contains(name.as_str()) {
                return builder.func_ret_ast.get(name).is_some_and(|ty| counted(builder, ty));
            }
        }
    }
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
