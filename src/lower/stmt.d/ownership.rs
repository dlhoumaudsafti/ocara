/// Destruction réelle de `scoped`/`consumed` (voir docs/EBNF.md et le plan
/// "Gestion de propriété des variables").
///
/// Ce module ne fait QUE l'émission des appels de libération/clonage au bon
/// endroit — la légalité (échappement interdit pour les ressources, type
/// pris en charge...) a déjà été validée par la sema
/// (`src/sema/typecheck.rs::check_escape`) ; ici on peut supposer qu'aucune
/// valeur `scoped`/`consumed` de type valeur ne s'échappe jamais (`var y = x`,
/// `y = x`, `return x`) sans passer par `maybe_clone_escaping` au préalable
/// (appelé depuis `lower_var`/`lower_assign`/`Stmt::Return` — PAS depuis un
/// argument d'appel, voir la doc de `maybe_clone_escaping` pour pourquoi).
///
/// Limite connue de ce premier chantier : une sortie de bloc anticipée
/// (`return`/`break`/`continue`/`raise`, y compris un `raise` qui traverse
/// un `try` via `longjmp` — voir `lower_try` dans
/// `src/lower/stmt.d/statements.d/exceptions.rs`) ne déclenche PAS la
/// destruction des `scoped`/`consumed` encore vivantes à ce point — elles
/// fuient (pas de use-after-free : rien d'autre ne peut aliaser leur
/// mémoire, voir `maybe_clone_escaping` — juste une fuite). Un vrai
/// rattrapage nécessiterait d'étendre le contrat runtime entre
/// `__ocara_fail`/`__ocara_try_exec` et le handler généré (aujourd'hui le
/// handler ne reçoit que `err_val`/`err_type`, pas d'accès aux locals du
/// corps `try` — dont la pile a de toute façon disparu au moment du
/// `longjmp`, il faudrait les faire vivre sur le tas comme le sont déjà les
/// *captures*). Chantier délibérément reporté : risque élevé sur un
/// mécanisme déjà fragile (voir le bug `Thread_join`/`longjmp` corrigé plus
/// tôt dans ce projet) pour un gain limité à un cas de fuite, jamais de
/// corruption.
use std::collections::HashMap;

use crate::ir::inst::{Inst, Value};
use crate::ir::types::IrType;
use crate::parsing::ast::{Block, Expr, Stmt, TemplatePartExpr, Type, VarKind};
use crate::sema::scope::{ownership_class, OwnershipClass};
use crate::lower::builder::LowerBuilder;

/// Point d'échappement (affectation, `return` — PAS un argument d'appel,
/// voir `TypeChecker::check_escape` côté sema pour la justification) côté
/// lowering. Si `expr` est un identifiant référant à une `scoped`/
/// `consumed` de type valeur (array/map — jamais string, voir
/// `OwnershipClass::Value`), clone `val` avant qu'il ne s'échappe : la
/// cible reçoit une copie indépendante, la source reste intacte et sera
/// détruite normalement à son propre point de destruction. Les ressources
/// ne peuvent jamais arriver ici avec `val` inchangé : la sema a déjà
/// refusé leur échappement (`SemaError::ResourceEscape`) — un programme
/// qui l'atteindrait quand même n'aurait pas dû passer `--check`.
pub fn maybe_clone_escaping(builder: &mut LowerBuilder, expr: &Expr, val: Value) -> Value {
    let Expr::Ident(name, _) = expr else { return val };
    let Some(info) = builder.owned_locals.get(name) else { return val };
    if !matches!(info.kind, VarKind::Scoped | VarKind::Consumed) {
        return val;
    }
    if info.class != OwnershipClass::Value {
        return val;
    }
    let cloned = builder.new_value();
    builder.emit(Inst::Call {
        dest: Some(cloned.clone()),
        func: "__value_clone".into(),
        args: vec![val],
        ret_ty: IrType::Ptr,
    });
    cloned
}

/// Métadonnées de propriété d'une `scoped`/`consumed` — indexé par nom dans
/// `LowerBuilder::owned_locals`. N'existe que pour les types pris en charge
/// (`OwnershipClass::Value` ou `Resource` — voir `register_owned_local`) ;
/// `Thread` (déjà libérée par `.join()`/`.detach()`, validé par la sema) et
/// `Unsupported` (se comporte comme `var`) n'ont jamais d'entrée.
#[derive(Debug, Clone)]
pub struct OwnedLocalInfo {
    pub kind:    VarKind,
    pub class:   OwnershipClass,
    pub ty:      Type,
    /// Vrai une fois détruite (fin de bloc, ou juste après l'unique usage
    /// d'une `consumed`) — évite un double-appel de libération.
    pub dropped: bool,
}

/// Appelé depuis `lower_var` juste après la déclaration d'une `scoped`/
/// `consumed`. Enregistre les métadonnées de propriété si le type est pris
/// en charge — sinon (Thread, type non supporté) ne fait rien.
pub fn register_owned_local(builder: &mut LowerBuilder, name: &str, ty: &Type, kind: VarKind) {
    if !matches!(kind, VarKind::Scoped | VarKind::Consumed) {
        return;
    }
    let class = ownership_class(ty);
    if matches!(class, OwnershipClass::Value | OwnershipClass::Resource) {
        builder.owned_locals.insert(
            name.to_string(),
            OwnedLocalInfo { kind, class, ty: ty.clone(), dropped: false },
        );
    }
}

/// Fonction runtime de libération pour un type valeur/ressource pris en
/// charge — `None` si ce type précis n'a finalement pas de destructeur
/// connu (ne devrait pas arriver pour une entrée déjà filtrée par
/// `register_owned_local`, mais on reste défensif).
///
/// Types valeur : `array`/`map` sont toujours de vraies allocations tas par
/// construction (`__array_new`/`__map_new`, jamais de littéral figé en
/// mémoire statique comme pour `string` — voir `OwnershipClass::Value`,
/// c'est justement pourquoi `string` n'atteint jamais cette branche).
/// `__value_free` gère aussi la récursion sur les éléments imbriqués.
fn drop_func_for(info: &OwnedLocalInfo) -> Option<&'static str> {
    match info.class {
        OwnershipClass::Value => match &info.ty {
            Type::Array(_) | Type::Map(_, _) => Some("__value_free"),
            _ => None,
        },
        OwnershipClass::Resource => match &info.ty {
            Type::Named(n) if n == "Mutex"    => Some("Mutex_destroy"),
            Type::Named(n) if n == "SQLite"   => Some("SQLite_close"),
            Type::Named(n) if n == "MySQL"    => Some("MySQL_close"),
            Type::Named(n) if n == "MariaDB"  => Some("MariaDB_close"),
            _ => None,
        },
        OwnershipClass::Thread | OwnershipClass::Unsupported => None,
    }
}

/// Détruit `name` si elle est encore possédée (pas déjà détruite) — recharge
/// sa valeur courante depuis son slot et émet l'appel de libération adapté.
fn emit_drop_if_owned(builder: &mut LowerBuilder, name: &str) {
    let Some(info) = builder.owned_locals.get(name).cloned() else { return };
    if info.dropped {
        return;
    }
    let Some(func) = drop_func_for(&info) else { return };
    let Some((val, _)) = builder.load_local(name) else { return };
    builder.emit(Inst::Call { dest: None, func: func.into(), args: vec![val], ret_ty: IrType::Void });
    if let Some(entry) = builder.owned_locals.get_mut(name) {
        entry.dropped = true;
    }
}

/// Détruit, dans l'ordre inverse de déclaration, toutes les `scoped`/
/// `consumed` déclarées DIRECTEMENT dans `block` (pas les blocs imbriqués,
/// qui gèrent les leurs via leur propre appel à `lower_block`) et pas
/// encore détruites. Appelé par `lower_block` à la fin d'un bloc qui se
/// termine normalement (pas de `return`/`raise`/`break`/`continue` déjà émis).
pub fn emit_scope_drops(builder: &mut LowerBuilder, block: &Block) {
    let names: Vec<String> = block.stmts.iter().rev().filter_map(|s| {
        if let Stmt::Var { name, kind: VarKind::Scoped | VarKind::Consumed, .. } = s {
            Some(name.clone())
        } else {
            None
        }
    }).collect();
    for name in names {
        emit_drop_if_owned(builder, &name);
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
    let mut found: Vec<String> = Vec::new();
    collect_consumed_reads_stmt(stmt, &builder.owned_locals, &mut found);
    for name in found {
        emit_drop_if_owned(builder, &name);
    }
}

fn is_live_consumed(name: &str, owned: &HashMap<String, OwnedLocalInfo>) -> bool {
    owned.get(name).is_some_and(|i| i.kind == VarKind::Consumed && !i.dropped)
}

/// Ne descend QUE dans les expressions directement portées par `stmt` — pas
/// dans les `Block` imbriqués (if/while/for/switch/try), qui sont scannés
/// séparément par leur propre passage dans `lower_block`.
fn collect_consumed_reads_stmt(stmt: &Stmt, owned: &HashMap<String, OwnedLocalInfo>, out: &mut Vec<String>) {
    match stmt {
        Stmt::Var { value, .. } => collect_consumed_reads_expr(value, owned, out),
        Stmt::Const { value, .. } => collect_consumed_reads_expr(value, owned, out),
        Stmt::Expr(e) => collect_consumed_reads_expr(e, owned, out),
        Stmt::Assign { target, value, .. } => {
            collect_consumed_reads_expr(target, owned, out);
            collect_consumed_reads_expr(value, owned, out);
        }
        Stmt::Return { value: Some(e), .. } | Stmt::Result { value: Some(e), .. } => {
            collect_consumed_reads_expr(e, owned, out);
        }
        Stmt::Return { .. } | Stmt::Result { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {}
        Stmt::If { condition, .. } => collect_consumed_reads_expr(condition, owned, out),
        Stmt::While { condition, .. } => collect_consumed_reads_expr(condition, owned, out),
        Stmt::ForIn { iter, .. } => collect_consumed_reads_expr(iter, owned, out),
        Stmt::ForMap { iter, .. } => collect_consumed_reads_expr(iter, owned, out),
        Stmt::Switch { subject, .. } => collect_consumed_reads_expr(subject, owned, out),
        Stmt::Try { .. } => {}
        Stmt::Raise { value, .. } => collect_consumed_reads_expr(value, owned, out),
    }
}

fn collect_consumed_reads_expr(expr: &Expr, owned: &HashMap<String, OwnedLocalInfo>, out: &mut Vec<String>) {
    match expr {
        Expr::Ident(name, _) => {
            if is_live_consumed(name, owned) && !out.contains(name) {
                out.push(name.clone());
            }
        }
        Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::Literal(..) | Expr::StaticConst { .. } => {}
        Expr::Binary { left, right, .. } => {
            collect_consumed_reads_expr(left, owned, out);
            collect_consumed_reads_expr(right, owned, out);
        }
        Expr::Unary { operand, .. } => collect_consumed_reads_expr(operand, owned, out),
        Expr::Field { object, .. } => collect_consumed_reads_expr(object, owned, out),
        Expr::Call { callee, args, .. } => {
            collect_consumed_reads_expr(callee, owned, out);
            for a in args { collect_consumed_reads_expr(a, owned, out); }
        }
        Expr::StaticCall { args, .. } => { for a in args { collect_consumed_reads_expr(a, owned, out); } }
        Expr::New { args, .. } => { for a in args { collect_consumed_reads_expr(a, owned, out); } }
        Expr::Index { object, index, .. } => {
            collect_consumed_reads_expr(object, owned, out);
            collect_consumed_reads_expr(index, owned, out);
        }
        Expr::Range { start, end, .. } => {
            collect_consumed_reads_expr(start, owned, out);
            collect_consumed_reads_expr(end, owned, out);
        }
        Expr::Array { elements, .. } => { for e in elements { collect_consumed_reads_expr(e, owned, out); } }
        Expr::Map { entries, .. } => {
            for (k, v) in entries {
                collect_consumed_reads_expr(k, owned, out);
                collect_consumed_reads_expr(v, owned, out);
            }
        }
        Expr::Template { parts, .. } => {
            for part in parts {
                if let TemplatePartExpr::Expr(e) = part { collect_consumed_reads_expr(e, owned, out); }
            }
        }
        Expr::Match { subject, arms, .. } => {
            collect_consumed_reads_expr(subject, owned, out);
            for arm in arms { collect_consumed_reads_expr(&arm.body, owned, out); }
        }
        Expr::IsCheck { expr, .. } => collect_consumed_reads_expr(expr, owned, out),
        Expr::Resolve { expr, .. } => collect_consumed_reads_expr(expr, owned, out),
        // Ne pas descendre dans les nameless imbriquées : une consumed
        // utilisée à l'intérieur d'une closure échapperait de toute façon
        // (interdit par check_escape côté sema pour les ressources ; pour
        // les types valeur, capturée par valeur au moment de la création
        // de la closure — pas un usage direct ici).
        Expr::Nameless { .. } => {}
    }
}
