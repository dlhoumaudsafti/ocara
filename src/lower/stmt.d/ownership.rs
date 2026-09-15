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
    let Some(strategy) = clone_func_for(builder.module, &info) else { return val };
    let cloned = builder.new_value();
    emit_ownership_call(builder, &strategy, val, Some(cloned.clone()), IrType::Ptr);
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
    /// `builder.loop_depth` au moment de la déclaration — voir sa doc dans
    /// `builder.d/types.rs`. Permet à `drop_consumed_used_in` de détecter
    /// qu'un usage se trouve dans un corps de boucle plus profond que la
    /// déclaration (donc répété à l'exécution), et de ne PAS y libérer la
    /// valeur : `emit_scope_drops`, au retour à la profondeur de boucle de
    /// déclaration, s'en charge une seule fois.
    pub declared_loop_depth: usize,
}

/// Appelé depuis `lower_var` juste après la déclaration d'une `scoped`/
/// `consumed`/`var`. Enregistre les métadonnées de propriété si le type est
/// pris en charge — sinon (Thread, type non supporté) ne fait rien.
///
/// Un `var` (kind normalement jamais libéré) est traité EXACTEMENT comme un
/// `scoped` implicite — MÊME mécanisme de libération en fin de bloc, aucun
/// codegen nouveau — quand `builder.auto_freeable_vars` (calculé une fois
/// par fonction/méthode, voir `compute_auto_freeable_vars` ci-dessous)
/// contient son nom, c'est-à-dire quand `crate::sema::escape::var_never_escapes`
/// a prouvé qu'il ne s'échappe jamais. Volontairement restreint à
/// `OwnershipClass::Value` (jamais `Resource`/`Thread` : fermer
/// implicitement une connexion DB/un mutex serait un changement de
/// comportement bien plus surprenant pour un simple `var`) — voir
/// docs/roadmap.d/memoire-strategie-var.md.
pub fn register_owned_local(builder: &mut LowerBuilder, name: &str, ty: &Type, kind: VarKind) {
    let class = ownership_class(ty);
    let effective_kind = match kind {
        VarKind::Scoped | VarKind::Consumed => kind,
        VarKind::Var => {
            if class == OwnershipClass::Value && builder.auto_freeable_vars.contains(name) {
                VarKind::Scoped
            } else {
                return;
            }
        }
    };
    if matches!(class, OwnershipClass::Value | OwnershipClass::Resource) {
        builder.owned_locals.insert(
            name.to_string(),
            OwnedLocalInfo { kind: effective_kind, class, ty: ty.clone(), dropped: false, declared_loop_depth: builder.loop_depth },
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
/// `true` pour un type d'élément PRIMITIF CONCRET (int/float/bool) — jamais
/// pour `mixed`, `string`, ou un type composite/nommé. Un élément primitif ne
/// possède jamais de mémoire propre : recurser dedans (`__value_free`/
/// `__value_clone` par élément) n'a rien à faire de plus qu'une copie brute,
/// et est dangereux (voir `drop_func_for`/`clone_func_for`) puisque son bit
/// pattern brut peut ressembler à un pointeur heap valide.
fn is_concrete_primitive_elem(ty: &Type) -> bool {
    matches!(ty, Type::Int | Type::Float | Type::Bool)
}

/// Calcule, pour un `array<T>`/`map<K,T>` dont l'élément `T` est lui-même un
/// `array`/`map` (imbriqué à une profondeur arbitraire), la "forme" de ses
/// ÉLÉMENTS : une séquence de caractères, un par niveau d'imbrication SOUS
/// le niveau courant (`'A'` = array, `'M'` = map), une chaîne vide signifiant
/// "élément terminal, un primitif concret sans mémoire propre". `None` si la
/// forme n'est PAS entièrement concrète à tous les niveaux (un `string`/
/// `mixed`/classe apparaît quelque part) — dans ce cas, le conteneur retombe
/// sur le chemin générique existant (`__value_free`/`__value_clone`),
/// correct pour un vrai pointeur heap à n'importe quel niveau.
///
/// Généralise `is_concrete_primitive_elem` (qui ne regardait qu'un seul
/// niveau) à une profondeur arbitraire : `array<array<int>>`/`array<array<
/// array<float>>>` retombaient auparavant sur le chemin générique dès le
/// PREMIER niveau imbriqué, qui redevient dangereux un niveau plus loin —
/// même bug que celui corrigé pour le niveau immédiat, seulement plus
/// profond (voir docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md et
/// `__array_free_concrete`/`__map_free_concrete` dans runtime/src/lib.rs).
fn concrete_elem_shape(container_ty: &Type) -> Option<String> {
    let elem_ty = match container_ty {
        Type::Array(inner) => inner.as_ref(),
        Type::Map(_, inner) => inner.as_ref(),
        _ => return None,
    };
    serialize_concrete_shape(elem_ty)
}

fn serialize_concrete_shape(ty: &Type) -> Option<String> {
    match ty {
        Type::Int | Type::Float | Type::Bool => Some(String::new()),
        Type::Array(inner) => serialize_concrete_shape(inner).map(|rest| format!("A{}", rest)),
        Type::Map(_, inner) => serialize_concrete_shape(inner).map(|rest| format!("M{}", rest)),
        _ => None,
    }
}

/// Stratégie de libération/clonage choisie par `drop_func_for`/`clone_func_for`.
enum OwnershipFunc {
    /// Appel à un seul argument (`val`) — le cas historique.
    Simple(String),
    /// Conteneur concret imbriqué sur 2+ niveaux (voir `concrete_elem_shape`) :
    /// appel à `(val, shape, 0)`, `shape` étant une string à interner dans le
    /// module — voir `emit_ownership_call`.
    ConcreteRecursive(String, String),
}

fn drop_func_for(module: &IrModule, info: &OwnedLocalInfo) -> Option<OwnershipFunc> {
    match info.class {
        OwnershipClass::Value => match &info.ty {
            // `array`/`map` à élément primitif concret : jamais de pointeur
            // heap à inspecter parmi les éléments — variante "shallow" (pas
            // de parcours récursif) obligatoire, voir sa doc dans
            // runtime/src/lib.rs. Corrige un SEGFAULT confirmé (`var
            // floats:array<float> = [1.5, 2.5, 3.5]`, jamais échappé : le
            // bit pattern brut d'un `float` ressemble parfois à un pointeur
            // heap valide, `__value_free` par élément le déréférençait).
            Type::Array(elem) if is_concrete_primitive_elem(elem) =>
                Some(OwnershipFunc::Simple("__array_free_shallow".to_string())),
            Type::Map(_, elem) if is_concrete_primitive_elem(elem) =>
                Some(OwnershipFunc::Simple("__map_free_shallow".to_string())),
            // Élément lui-même array/map, à une profondeur arbitraire — voir
            // `concrete_elem_shape`/`__array_free_concrete`.
            Type::Array(_) => match concrete_elem_shape(&info.ty) {
                Some(shape) => Some(OwnershipFunc::ConcreteRecursive("__array_free_concrete".to_string(), shape)),
                None => Some(OwnershipFunc::Simple("__value_free".to_string())),
            },
            Type::Map(_, _) => match concrete_elem_shape(&info.ty) {
                Some(shape) => Some(OwnershipFunc::ConcreteRecursive("__map_free_concrete".to_string(), shape)),
                None => Some(OwnershipFunc::Simple("__value_free".to_string())),
            },
            Type::String => Some(OwnershipFunc::Simple("__value_free".to_string())),
            Type::Named(n) if class_ownership::has_generated_destructor(module, n) => {
                Some(OwnershipFunc::Simple(format!("__free_{}", n)))
            }
            _ => None,
        },
        OwnershipClass::Resource => match &info.ty {
            Type::Named(n) if n == "Mutex"       => Some(OwnershipFunc::Simple("Mutex_destroy".to_string())),
            Type::Named(n) if n == "SQLite"      => Some(OwnershipFunc::Simple("SQLite_close".to_string())),
            Type::Named(n) if n == "MySQL"       => Some(OwnershipFunc::Simple("MySQL_close".to_string())),
            Type::Named(n) if n == "MariaDB"     => Some(OwnershipFunc::Simple("MariaDB_close".to_string())),
            Type::Named(n) if n == "HTTPRequest"  => Some(OwnershipFunc::Simple("HTTPRequest_close".to_string())),
            Type::Named(n) if n == "HTTPResponse" => Some(OwnershipFunc::Simple("HTTPRequest_closeResponse".to_string())),
            _ => None,
        },
        OwnershipClass::Thread | OwnershipClass::Unsupported => None,
    }
}

/// Comme `drop_func_for`, pour le clonage à l'échappement (voir
/// `maybe_clone_escaping`) — uniquement pertinent pour `OwnershipClass::Value`
/// (les ressources ne s'échappent jamais, refusé par la sema).
fn clone_func_for(module: &IrModule, info: &OwnedLocalInfo) -> Option<OwnershipFunc> {
    match &info.ty {
        // Voir `drop_func_for` : même raison de choisir la variante "shallow".
        Type::Array(elem) if is_concrete_primitive_elem(elem) =>
            Some(OwnershipFunc::Simple("__array_clone_shallow".to_string())),
        Type::Map(_, elem) if is_concrete_primitive_elem(elem) =>
            Some(OwnershipFunc::Simple("__map_clone_shallow".to_string())),
        Type::Array(_) => match concrete_elem_shape(&info.ty) {
            Some(shape) => Some(OwnershipFunc::ConcreteRecursive("__array_clone_concrete".to_string(), shape)),
            None => Some(OwnershipFunc::Simple("__value_clone".to_string())),
        },
        Type::Map(_, _) => match concrete_elem_shape(&info.ty) {
            Some(shape) => Some(OwnershipFunc::ConcreteRecursive("__map_clone_concrete".to_string(), shape)),
            None => Some(OwnershipFunc::Simple("__value_clone".to_string())),
        },
        Type::String => Some(OwnershipFunc::Simple("__value_clone".to_string())),
        Type::Named(n) if class_ownership::has_generated_destructor(module, n) => {
            Some(OwnershipFunc::Simple(format!("__clone_{}", n)))
        }
        _ => None,
    }
}

/// Émet l'appel de libération/clonage décrit par `strategy`, avec `val`
/// comme premier argument — pour `ConcreteRecursive`, complète avec la
/// string de forme (internée dans le module) et l'offset initial (0), voir
/// `__array_free_concrete`/`__map_free_concrete`/`__array_clone_concrete`/
/// `__map_clone_concrete` dans runtime/src/lib.rs.
fn emit_ownership_call(builder: &mut LowerBuilder, strategy: &OwnershipFunc, val: Value, dest: Option<Value>, ret_ty: IrType) {
    let (func, args) = match strategy {
        OwnershipFunc::Simple(name) => (name.clone(), vec![val]),
        OwnershipFunc::ConcreteRecursive(name, shape) => {
            let idx = builder.module.intern_string(shape);
            let shape_val = builder.new_value();
            builder.emit(Inst::ConstStr { dest: shape_val.clone(), idx });
            let offset_val = builder.new_value();
            builder.emit(Inst::ConstInt { dest: offset_val.clone(), value: 0 });
            (name.clone(), vec![val, shape_val, offset_val])
        }
    };
    builder.emit(Inst::Call { dest, func, args, ret_ty });
}

/// Détruit `name` si elle est encore possédée (pas déjà détruite) — recharge
/// sa valeur courante depuis son slot et émet l'appel de libération adapté.
fn emit_drop_if_owned(builder: &mut LowerBuilder, name: &str) {
    let Some(info) = builder.owned_locals.get(name).cloned() else { return };
    if info.dropped {
        return;
    }
    let Some(strategy) = drop_func_for(builder.module, &info) else { return };
    let Some((val, _)) = builder.load_local(name) else { return };
    emit_ownership_call(builder, &strategy, val, None, IrType::Void);
    if let Some(entry) = builder.owned_locals.get_mut(name) {
        entry.dropped = true;
    }
}

/// Appelé depuis `lower_assign` juste avant de stocker une nouvelle valeur
/// dans une variable `scoped`/`consumed` déjà déclarée (`s = nouvelleValeur`)
/// — libère l'ANCIENNE valeur qu'elle contenait, sinon elle fuit (remplacée
/// sans jamais être libérée : confirmé par reproduction, voir
/// `docs/roadmap.d/memoire-double-free-et-fuites-scoped.md`). Ne touche pas
/// `dropped` : la variable reste possédée, la nouvelle valeur qu'elle va
/// recevoir sera libérée normalement à son propre point de destruction.
/// Uniquement pour `OwnershipClass::Value` (string/array/map/classe
/// utilisateur) — une ressource ne peut pas être réaffectée (échappement
/// refusé par la sema), rien à faire ici pour ce cas.
pub fn free_before_reassign(builder: &mut LowerBuilder, name: &str) {
    let Some(info) = builder.owned_locals.get(name).cloned() else { return };
    if info.class != OwnershipClass::Value || info.dropped {
        return;
    }
    let Some(strategy) = drop_func_for(builder.module, &info) else { return };
    let Some((val, _)) = builder.load_local(name) else { return };
    emit_ownership_call(builder, &strategy, val, None, IrType::Void);
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
    let mut found: Vec<String> = Vec::new();
    collect_consumed_reads_stmt(stmt, &builder.owned_locals, &mut found);
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
        Stmt::Emit { value, .. } => collect_consumed_reads_expr(value, owned, out),
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

/// Calcule, pour un corps de fonction/méthode/constructeur, l'ensemble des
/// noms de `var` (jamais `scoped`/`consumed`, déjà explicites) prouvés ne
/// jamais s'échapper (voir `crate::sema::escape::var_never_escapes`) —
/// appelé une fois avant de lowered le corps (voir `lower_func`), stocké
/// dans `builder.auto_freeable_vars`, consulté par `register_owned_local`.
///
/// Restreint à `OwnershipClass::Value` (string/array/map/classe utilisateur
/// avec destructeur généré) : jamais `Resource`/`Thread`/`Unsupported` — un
/// `var` sur ces types continue de se comporter exactement comme aujourd'hui
/// (voir docs/roadmap.d/memoire-strategie-var.md).
pub fn compute_auto_freeable_vars(module: &IrModule, body: &Block, self_class: Option<&str>) -> std::collections::HashSet<String> {
    let mut eligible = std::collections::HashSet::new();
    collect_var_candidates(module, body, self_class, &mut eligible);
    eligible
}

fn collect_var_candidates(
    module: &IrModule, block: &Block, self_class: Option<&str>,
    eligible: &mut std::collections::HashSet<String>,
) {
    for (i, stmt) in block.stmts.iter().enumerate() {
        if let Stmt::Var { name, ty, value, kind: VarKind::Var, .. } = stmt {
            if ownership_class(ty) == OwnershipClass::Value
                && is_fresh_allocation(value)
                && crate::sema::escape::var_never_escapes(&module.class_members, name, block, i, self_class, &module.escaping_params)
            {
                eligible.insert(name.clone());
            }
        }
        walk_nested_blocks_for_vars(stmt, module, self_class, eligible);
    }
}

/// Un `var` n'est éligible à la libération automatique que si son
/// initialiseur produit une valeur FRAÎCHEMENT allouée et possédée
/// UNIQUEMENT par ce `var` — sinon, même si le `var` lui-même ne s'échappe
/// jamais après coup, sa valeur pourrait déjà être un ALIAS partagé avec
/// autre chose (ex. `var got:Holder = acc.get(j)` : `got` et `acc[j]`
/// pointent vers le MÊME objet — libérer `got` en fin de bloc alors que
/// `acc` le référence encore est un double free confirmé par reproduction).
///
/// Formes reconnues comme sûres :
/// - `use Classe(...)` : toujours une allocation neuve
///   (`__alloc_class_obj`), quels que soient ses arguments.
/// - Littéral tableau/map (`[...]`/`{...}`) : toujours une structure neuve
///   (`__array_new`/`__map_new`) ; un élément qui serait lui-même un alias
///   taintée est déjà marqué comme échappant par
///   `crate::sema::escape::walk_expr_for_calls` (transfert de propriété
///   correctement pris en compte).
/// - Concaténation `+` : `__str_concat` (runtime/src/lib.rs) alloue
///   systématiquement une nouvelle string, jamais un des deux opérandes
///   inchangé.
/// - Littéral simple (int/float/bool/string/null) : soit non possédable
///   (primitif), soit un littéral string (`TAG_STRING`, jamais
///   `TAG_STRING_OWNED`) — le libérer est un no-op garanti (`is_owned_string`).
///
/// Tout le reste (n'importe quel appel — `.get()`, `Convert::*`,
/// `String::*`, accès de champ, accès indexé, `match`...) est exclu par
/// prudence : on ne peut pas garantir en général qu'un appel ne retourne
/// pas un pointeur déjà possédé ailleurs.
fn is_fresh_allocation(value: &Expr) -> bool {
    match value {
        Expr::New { .. } | Expr::Array { .. } | Expr::Map { .. } | Expr::Literal(..) => true,
        Expr::Binary { op: crate::parsing::ast::BinOp::Add, .. } => true,
        _ => false,
    }
}

/// Descend dans les blocs imbriqués d'un statement (if/switch/while/for/try)
/// pour y trouver D'AUTRES `var` candidats — chacun vérifié par rapport à
/// SON PROPRE bloc englobant, indépendamment de ceux du bloc parent.
fn walk_nested_blocks_for_vars(
    stmt: &Stmt, module: &IrModule, self_class: Option<&str>,
    eligible: &mut std::collections::HashSet<String>,
) {
    match stmt {
        Stmt::If { then_block, elseif, else_block, .. } => {
            collect_var_candidates(module, then_block, self_class, eligible);
            for (_, b) in elseif { collect_var_candidates(module, b, self_class, eligible); }
            if let Some(b) = else_block { collect_var_candidates(module, b, self_class, eligible); }
        }
        Stmt::Switch { cases, default, .. } => {
            for c in cases { collect_var_candidates(module, &c.body, self_class, eligible); }
            if let Some(b) = default { collect_var_candidates(module, b, self_class, eligible); }
        }
        Stmt::While { body, .. } => collect_var_candidates(module, body, self_class, eligible),
        Stmt::ForIn { body, .. } => collect_var_candidates(module, body, self_class, eligible),
        Stmt::ForMap { body, .. } => collect_var_candidates(module, body, self_class, eligible),
        Stmt::Try { body, handlers, .. } => {
            collect_var_candidates(module, body, self_class, eligible);
            for h in handlers { collect_var_candidates(module, &h.body, self_class, eligible); }
        }
        Stmt::Var { .. } | Stmt::Const { .. } | Stmt::Expr(_) | Stmt::Return { .. } | Stmt::Result { .. }
        | Stmt::Break { .. } | Stmt::Continue { .. } | Stmt::Raise { .. } | Stmt::Assign { .. }
        | Stmt::Emit { .. } => {}
    }
}
