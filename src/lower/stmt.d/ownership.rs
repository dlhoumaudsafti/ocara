/// Fermeture des ressources `scoped`/`consumed` (connexion, fichier,
/// mutex… — `OwnershipClass::Resource`), voir docs/EBNF.md. La mémoire des
/// valeurs relève du comptage de références (`crate::lower::stmt::rc`).
///
/// `return`/`break`/`continue` anticipés ferment aussi les ressources des
/// blocs qu'ils traversent (`emit_early_exit_drops`, `block_scope_stack`).
/// Chaque ressource ouverte est enregistrée auprès du runtime
/// (`__rc_resource_push`) : un `raise` ferme celles des frames qu'il saute.
use std::collections::HashMap;

use crate::ir::inst::Inst;
use crate::ir::types::IrType;
use crate::parsing::ast::{Expr, Stmt, Block, TemplatePartExpr, Type, VarKind};
use crate::sema::scope::{ownership_class, OwnershipClass};
use crate::lower::builder::LowerBuilder;

/// Ressource `scoped`/`consumed` vivante, indexée par nom dans
/// `LowerBuilder::owned_locals`.
#[derive(Debug, Clone)]
pub struct OwnedLocalInfo {
    pub kind:    VarKind,
    pub ty:      Type,
    /// Vrai une fois fermée — évite une double fermeture.
    pub dropped: bool,
    /// `builder.loop_depth` à la déclaration : un usage `consumed` dans une
    /// boucle plus profonde ne ferme pas la ressource (elle serait fermée à
    /// chaque itération), `emit_scope_drops` s'en charge.
    pub declared_loop_depth: usize,
    /// Slot enregistré pour la fermeture par déroulement (`raise`).
    pub unwind_slot: Option<crate::ir::inst::Value>,
}

/// Appelé par `lower_var` : enregistre une ressource `scoped`/`consumed`.
pub fn register_owned_local(builder: &mut LowerBuilder, name: &str, ty: &Type, kind: VarKind) {
    if !matches!(kind, VarKind::Scoped | VarKind::Consumed) {
        return;
    }
    if ownership_class(ty) != OwnershipClass::Resource {
        return;
    }
    let mut info = OwnedLocalInfo { kind, ty: ty.clone(), dropped: false, declared_loop_depth: builder.loop_depth, unwind_slot: None };
    info.unwind_slot = register_unwind(builder, name, &info);
    builder.owned_locals.insert(name.to_string(), info);
    if let Some(frame) = builder.block_scope_stack.last_mut() {
        frame.push(name.to_string());
    }
}

/// Enregistre la ressource (slot, fonction de fermeture) pour qu'un `raise`
/// qui traverse la fonction la ferme.
fn register_unwind(builder: &mut LowerBuilder, name: &str, info: &OwnedLocalInfo) -> Option<crate::ir::inst::Value> {
    if builder.rc_generator || builder.frame_vars.contains_key(name) || builder.heap_promoted.contains(name) {
        return None;
    }
    let closer = closer_for(info)?;
    let (slot, _, _) = builder.locals.get(name).cloned()?;
    let addr = builder.new_value();
    builder.emit(Inst::FuncAddr { dest: addr.clone(), func: closer });
    builder.emit(Inst::Call { dest: None, func: "__rc_resource_push".into(), args: vec![slot.clone(), addr], ret_ty: IrType::Void });
    Some(slot)
}

/// La ressource `name` est fermée (fin de portée ou fermeture explicite) :
/// plus de fermeture automatique, ni par le bloc, ni par un `raise`.
pub fn mark_finalized(builder: &mut LowerBuilder, name: &str) {
    let Some(info) = builder.owned_locals.get_mut(name) else { return };
    info.dropped = true;
    if let Some(slot) = info.unwind_slot.clone() {
        builder.emit(Inst::Call { dest: None, func: "__rc_resource_pop".into(), args: vec![slot], ret_ty: IrType::Void });
    }
}

fn closer_for(info: &OwnedLocalInfo) -> Option<String> {
    match &info.ty {
        Type::Named(n) => crate::sema::scope::resource_closer_symbol(n).map(str::to_string),
        _ => None,
    }
}

/// Détruit `name` si elle est encore possédée (pas déjà détruite) — recharge
/// sa valeur courante depuis son slot et émet l'appel de libération adapté.
fn emit_drop_if_owned(builder: &mut LowerBuilder, name: &str) {
    let Some(info) = builder.owned_locals.get(name).cloned() else { return };
    if info.dropped {
        return;
    }
    let Some(func) = closer_for(&info) else { return };
    let Some((val, _)) = builder.load_local(name) else { return };
    builder.emit(Inst::Call { dest: None, func, args: vec![val], ret_ty: IrType::Void });
    mark_finalized(builder, name);
}

/// Détruit, dans l'ordre inverse de déclaration, toutes les `scoped`/
/// `consumed` déclarées DIRECTEMENT dans `block` (pas les blocs imbriqués,
/// qui gèrent les leurs via leur propre appel à `lower_block`) et pas
/// encore détruites. Appelé par `lower_block` à la fin d'un bloc qui se
/// termine normalement (pas de `return`/`raise`/`break`/`continue` déjà émis).
pub fn emit_scope_drops(builder: &mut LowerBuilder, block: &Block) {
    // Filtre sur `owned_locals` (pas directement le `kind` AST) : un `var`
    // prouvé non-échappant y est enregistré exactement comme un `scoped`
    // (voir `register_owned_local`) — son `kind` AST reste `Var`, seul
    // `owned_locals` sait qu'il doit être libéré ici.
    let names: Vec<String> = block.stmts.iter().rev().filter_map(|s| {
        if let Stmt::Var { name, .. } = s {
            if builder.owned_locals.contains_key(name) {
                return Some(name.clone());
            }
        }
        None
    }).collect();
    for name in names {
        emit_drop_if_owned(builder, &name);
    }
}

/// Détruit toutes les `scoped`/`consumed` encore vivantes dans les blocs
/// actuellement ouverts, depuis le plus interne (`block_scope_stack.len() -
/// 1`) jusqu'à `down_to_depth` INCLUS — appelé juste avant d'émettre le
/// terminateur d'un `return`/`break`/`continue` anticipé, pour que sortir
/// tôt d'un bloc ne fasse plus fuir ce qui y était encore possédé.
///
/// `down_to_depth` :
///   - `0` pour `return` — sort de toute la fonction, tout est concerné.
///   - la profondeur de `block_scope_stack` au moment où la boucle courante
///     a été entrée (voir `loop_stack`) pour `break`/`continue` — sort du
///     corps de la boucle (inclus) mais pas des blocs qui l'englobent.
///
/// Sans effet sur les blocs eux-mêmes : `lower_block` fait toujours son
/// propre `pop()` normalement juste après (`is_terminated()` protège son
/// `emit_scope_drops` contre un double-appel, voir sa doc).
pub fn emit_early_exit_drops(builder: &mut LowerBuilder, down_to_depth: usize) {
    let len = builder.block_scope_stack.len();
    if down_to_depth >= len {
        return;
    }
    for i in (down_to_depth..len).rev() {
        let names = builder.block_scope_stack[i].clone();
        for name in names.iter().rev() {
            emit_drop_if_owned(builder, &name);
        }
    }
}

/// Appelé par `lower_block` juste après avoir lowered `stmt` : si `stmt`
/// contient (directement, sans descendre dans les blocs imbriqués — voir le
/// commentaire d'en-tête) l'unique usage permis d'une ou plusieurs
/// `consumed`, les détruit immédiatement. La sema (`check_escape` +
/// `use_binding`) garantit déjà qu'une `consumed` n'est utilisée qu'une
/// seule fois dans tout le programme — on peut donc détruire sans risquer
/// de couper court à un usage légitime ultérieur.
pub fn drop_consumed_used_in(builder: &mut LowerBuilder, stmt: &Stmt) {
    let owned = builder.owned_locals.clone();
    let found = consumed_reads(stmt, &|name| is_live_consumed(name, &owned));
    for name in found {
        // Usage situé dans un corps de boucle plus profond que la
        // déclaration (donc répété à chaque itération réelle) : ne pas
        // libérer ici, sous peine de libérer plusieurs fois la même valeur
        // au fil des itérations (confirmé par reproduction — voir
        // docs/roadmap.d/memoire-double-free-et-fuites-scoped.md).
        // `emit_scope_drops` libère correctement une seule fois, quand le
        // bloc où la variable est déclarée se termine normalement (donc
        // après la boucle, puisqu'elle la précède).
        let declared_loop_depth = builder.owned_locals.get(&name).map(|i| i.declared_loop_depth).unwrap_or(0);
        if builder.loop_depth > declared_loop_depth {
            continue;
        }
        emit_drop_if_owned(builder, &name);
    }
}

fn is_live_consumed(name: &str, owned: &HashMap<String, OwnedLocalInfo>) -> bool {
    owned.get(name).is_some_and(|i| i.kind == VarKind::Consumed && !i.dropped)
}

/// Variables `consumed` vivantes (`live`) lues par `stmt` lui-même.
pub(crate) fn consumed_reads(stmt: &Stmt, live: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut found = Vec::new();
    collect_consumed_reads_stmt(stmt, live, &mut found);
    found
}

/// Ne descend QUE dans les expressions directement portées par `stmt` — pas
/// dans les `Block` imbriqués (if/while/for/switch/try), qui sont scannés
/// séparément par leur propre passage dans `lower_block`.
fn collect_consumed_reads_stmt(stmt: &Stmt, live: &dyn Fn(&str) -> bool, out: &mut Vec<String>) {
    match stmt {
        Stmt::Var { value, .. } => collect_consumed_reads_expr(value, live, out),
        Stmt::Const { value, .. } => collect_consumed_reads_expr(value, live, out),
        Stmt::Expr(e) => collect_consumed_reads_expr(e, live, out),
        Stmt::Assign { target, value, .. } => {
            collect_consumed_reads_expr(target, live, out);
            collect_consumed_reads_expr(value, live, out);
        }
        Stmt::Return { value: Some(e), .. } | Stmt::Result { value: Some(e), .. } => {
            collect_consumed_reads_expr(e, live, out);
        }
        Stmt::Return { .. } | Stmt::Result { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {}
        Stmt::If { condition, .. } => collect_consumed_reads_expr(condition, live, out),
        Stmt::While { condition, .. } => collect_consumed_reads_expr(condition, live, out),
        Stmt::ForIn { iter, .. } => collect_consumed_reads_expr(iter, live, out),
        Stmt::ForMap { iter, .. } => collect_consumed_reads_expr(iter, live, out),
        Stmt::Switch { subject, .. } => collect_consumed_reads_expr(subject, live, out),
        Stmt::Try { .. } => {}
        Stmt::Raise { value, .. } => collect_consumed_reads_expr(value, live, out),
        Stmt::Emit { value, .. } => collect_consumed_reads_expr(value, live, out),
    }
}

fn collect_consumed_reads_expr(expr: &Expr, live: &dyn Fn(&str) -> bool, out: &mut Vec<String>) {
    match expr {
        Expr::Ident(name, _) => {
            if live(name) && !out.contains(name) {
                out.push(name.clone());
            }
        }
        Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::Literal(..) | Expr::StaticConst { .. } => {}
        Expr::Binary { left, right, .. } => {
            collect_consumed_reads_expr(left, live, out);
            collect_consumed_reads_expr(right, live, out);
        }
        Expr::Unary { operand, .. } => collect_consumed_reads_expr(operand, live, out),
        Expr::Field { object, .. } => collect_consumed_reads_expr(object, live, out),
        Expr::Call { callee, args, .. } => {
            collect_consumed_reads_expr(callee, live, out);
            for a in args { collect_consumed_reads_expr(a, live, out); }
        }
        Expr::StaticCall { args, .. } => { for a in args { collect_consumed_reads_expr(a, live, out); } }
        Expr::New { args, .. } => { for a in args { collect_consumed_reads_expr(a, live, out); } }
        Expr::Index { object, index, .. } => {
            collect_consumed_reads_expr(object, live, out);
            collect_consumed_reads_expr(index, live, out);
        }
        Expr::Range { start, end, .. } => {
            collect_consumed_reads_expr(start, live, out);
            collect_consumed_reads_expr(end, live, out);
        }
        Expr::Array { elements, .. } => { for e in elements { collect_consumed_reads_expr(e, live, out); } }
        Expr::Map { entries, .. } => {
            for (k, v) in entries {
                collect_consumed_reads_expr(k, live, out);
                collect_consumed_reads_expr(v, live, out);
            }
        }
        Expr::Template { parts, .. } => {
            for part in parts {
                if let TemplatePartExpr::Expr(e) = part { collect_consumed_reads_expr(e, live, out); }
            }
        }
        Expr::Match { subject, arms, .. } => {
            collect_consumed_reads_expr(subject, live, out);
            for arm in arms { collect_consumed_reads_expr(&arm.body, live, out); }
        }
        Expr::IsCheck { expr, .. } => collect_consumed_reads_expr(expr, live, out),
        Expr::Resolve { expr, .. } => collect_consumed_reads_expr(expr, live, out),
        // `i++`/`++i`/`i--`/`--i` — la cible est toujours `int`/`float`
        // (jamais `scoped`/`consumed`, sema l'a déjà rejeté), mais un `Index`
        // peut contenir une lecture `consumed` dans son `object`/`index`
        // (`arr[c.id]++` où `c` est `consumed`) — à descendre comme ailleurs.
        Expr::IncDec { target, .. } | Expr::NamedArg { value: target, .. } => collect_consumed_reads_expr(target, live, out),
        // Ne pas descendre dans les nameless imbriquées : une consumed
        // utilisée à l'intérieur d'une closure échapperait de toute façon
        // (interdit par check_escape côté sema pour les ressources ; pour
        // les types valeur, capturée par valeur au moment de la création
        // de la closure — pas un usage direct ici).
        Expr::Nameless { .. } => {}
    }
}
