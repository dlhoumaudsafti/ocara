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
/// `return`/`break`/`continue` anticipés détruisent eux aussi correctement
/// les `scoped`/`consumed` encore vivantes dans les blocs qu'ils traversent
/// — voir `emit_early_exit_drops` et `block_scope_stack`
/// (`src/lower/builder.d/types.rs`), câblés depuis `Stmt::Return`
/// (`src/lower/stmt.d/statements.rs`) et `lower_break`/`lower_continue`
/// (`src/lower/stmt.d/statements.d/loops.rs`).
///
/// Limite connue restante (délibérément reportée) : un `raise` qui traverse
/// un `try` englobant via `longjmp` (voir `lower_try` dans
/// `src/lower/stmt.d/statements.d/exceptions.rs`) échappe à tout ça — les
/// `scoped`/`consumed` encore vivantes à ce moment fuient (pas de
/// use-after-free : rien d'autre ne peut aliaser leur mémoire, voir
/// `maybe_clone_escaping` — juste une fuite). Un vrai rattrapage
/// nécessiterait d'étendre le contrat runtime entre
/// `__ocara_fail`/`__ocara_try_exec` et le handler généré (aujourd'hui le
/// handler ne reçoit que `err_val`/`err_type`, pas d'accès aux locals du
/// corps `try` — dont la pile a de toute façon disparu au moment du
/// `longjmp`, il faudrait les faire vivre sur le tas comme le sont déjà les
/// *captures*) : risque élevé sur un mécanisme déjà fragile (voir le bug
/// `Thread_join`/`longjmp` corrigé plus tôt dans ce projet) pour un gain
/// limité à un cas de fuite, jamais de corruption.
use std::collections::HashMap;

use crate::ir::inst::{Inst, Value};
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use crate::parsing::ast::{Block, Expr, Stmt, TemplatePartExpr, Type, VarKind};
use crate::sema::scope::{ownership_class, OwnershipClass};
use crate::lower::builder::LowerBuilder;
use crate::lower::builder::class_ownership;

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
    let Some(info) = builder.owned_locals.get(name).cloned() else { return val };
    if !matches!(info.kind, VarKind::Scoped | VarKind::Consumed) {
        return val;
    }
    if info.class != OwnershipClass::Value {
        return val;
    }
    // Type non reconnu (SDL/Tauri/toute classe sans __clone_ généré) :
    // aucun clone à faire — aliasé tel quel, comportement `var` inchangé.
    let Some(func) = clone_func_for(builder.module, &info) else { return val };
    let cloned = builder.new_value();
    builder.emit(Inst::Call {
        dest: Some(cloned.clone()),
        func,
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
        // Alimente block_scope_stack pour emit_early_exit_drops (return/
        // break/continue anticipés) — voir sa doc dans builder.d/types.rs.
        if let Some(frame) = builder.block_scope_stack.last_mut() {
            frame.push(name.to_string());
        }
    }
}

/// Fonction runtime de libération pour un type valeur/ressource pris en
/// charge — `None` si ce type précis n'a finalement pas de destructeur
/// connu (Thread/Unsupported déjà filtrés par `register_owned_local` ;
/// pour `Value`, une classe utilisateur sans `__free_` généré — voir
/// `has_generated_destructor` — se comporte comme `var`, ni erreur ni crash).
///
/// Types valeur non-classe : toujours `__value_free`, qui dispatche sur le
/// tag RUNTIME (pas ce type statique AST) — indispensable pour `string`, où
/// un littéral (tag `TAG_STRING`) et une allocation tas réelle (tag
/// `TAG_STRING_OWNED`) partagent le même type AST mais pas la même
/// libérabilité (voir `OwnershipClass::Value`). `array`/`map` passent par
/// la même fonction pour rester uniformes et gérer récursivement les
/// éléments imbriqués. Instance de classe utilisateur : `__free_<Classe>`
/// généré par `crate::lower::builder::class_ownership`.
fn drop_func_for(module: &IrModule, info: &OwnedLocalInfo) -> Option<String> {
    match info.class {
        OwnershipClass::Value => match &info.ty {
            Type::String | Type::Array(_) | Type::Map(_, _) => Some("__value_free".to_string()),
            Type::Named(n) if class_ownership::has_generated_destructor(module, n) => {
                Some(format!("__free_{}", n))
            }
            _ => None,
        },
        OwnershipClass::Resource => match &info.ty {
            Type::Named(n) if n == "Mutex"    => Some("Mutex_destroy".to_string()),
            Type::Named(n) if n == "SQLite"   => Some("SQLite_close".to_string()),
            Type::Named(n) if n == "MySQL"    => Some("MySQL_close".to_string()),
            Type::Named(n) if n == "MariaDB"  => Some("MariaDB_close".to_string()),
            _ => None,
        },
        OwnershipClass::Thread | OwnershipClass::Unsupported => None,
    }
}

/// Comme `drop_func_for`, pour le clonage à l'échappement (voir
/// `maybe_clone_escaping`) — uniquement pertinent pour `OwnershipClass::Value`
/// (les ressources ne s'échappent jamais, refusé par la sema).
fn clone_func_for(module: &IrModule, info: &OwnedLocalInfo) -> Option<String> {
    match &info.ty {
        Type::String | Type::Array(_) | Type::Map(_, _) => Some("__value_clone".to_string()),
        Type::Named(n) if class_ownership::has_generated_destructor(module, n) => {
            Some(format!("__clone_{}", n))
        }
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
    let Some(func) = drop_func_for(builder.module, &info) else { return };
    let Some((val, _)) = builder.load_local(name) else { return };
    builder.emit(Inst::Call { dest: None, func, args: vec![val], ret_ty: IrType::Void });
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
