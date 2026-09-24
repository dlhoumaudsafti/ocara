/// Lowering des déclarations de variables et constantes

use crate::parsing::ast::*;
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;
use crate::lower::expr::{lower_expr, expr_ir_type_pub};
use crate::core::monomorph::monomorphized_name;
use super::helpers::box_for_any;

/// Retourne le type de valeur d'une `map<K, V>` déclarée, en dépliant un
/// type union `map<K, V>|null` (ex. `MySQL::queryOne`) — un simple `if let
/// Type::Map(_, v) = ty` ne matche QUE la forme non-nullable directe. Sans
/// ce dépliage, une variable `map<string,mixed>|null` n'est jamais ajoutée à
/// `builder.map_vars`, donc `is_map_target` la voit comme "pas une map", et
/// `Expr::Index` dispatche silencieusement vers `__array_get` au lieu de
/// `__map_get` — confirmé par reproduction (`MySQL::queryOne`, seul builtin
/// du langage à retourner un `map<...>|null`, donc le seul endroit qui
/// exerçait ce chemin) : `db.queryOne(...)["champ"]` retournait `null` pour
/// CHAQUE champ après narrowing, alors que la ligne existait bien. Voir
/// docs/roadmap.d/stdlib-mysql-requetes-parametrees-transactions.md.
fn map_value_type(ty: &Type) -> Option<&Type> {
    match ty {
        Type::Map(_, val_ty) => Some(val_ty),
        Type::Union(variants) => variants.iter().find_map(map_value_type),
        _ => None,
    }
}

/// Enregistre dans `builder` les métadonnées dérivées du TYPE DÉCLARÉ `ty`
/// d'une variable `name` qui donnent accès à ses champs/méthodes d'instance
/// (`var_class`) ou en font une cible d'appel indirect (`func_vars`/
/// `func_ret_types`) — partagé par `lower_var` ET `lower_const` : les deux
/// déclarent une variable à partir d'un type écrit et doivent en dériver
/// EXACTEMENT les mêmes métadonnées (les cas `array<T>`/`map<K,V>` de base,
/// `elem_types`/`elem_ast_types`/`map_vars`, restent gérés séparément par
/// chaque appelant, avant cet appel, car ils précèdent aussi le calcul de
/// `map_value_type` ci-dessus).
///
/// Avant cette factorisation, `lower_const` réimplémentait sa propre copie
/// partielle de ce calcul — sans le cas `Type::Union` (`Classe|null`),
/// jamais ajouté ici contrairement à `lower_var`. Voir la doc de
/// `union_named_class` (src/parsing/ast.d/types.rs) pour le bug que ça
/// causait et docs/roadmap.d/langage-union-class-null-field-access.md pour
/// la reproduction complète.
fn register_var_class(builder: &mut LowerBuilder, name: &str, ty: &Type) {
    // Type de classe utilisateur direct.
    if let Type::Named(class_name) = ty {
        builder.var_class.insert(name.to_string(), class_name.clone());
    }

    // Générique : utiliser le nom monomorphisé (même résolution qu'un
    // paramètre de fonction, voir functions.rs).
    if let Type::Generic { name: generic_name, args } = ty {
        let specialized_name = monomorphized_name(generic_name, args);
        builder.var_class.insert(name.to_string(), specialized_name);
    }

    // Les variables string/array/map ont automatiquement accès aux méthodes
    // de leur classe builtin respective.
    if let Type::String = ty {
        builder.var_class.insert(name.to_string(), "String".to_string());
    }
    if let Type::Array(_) = ty {
        builder.var_class.insert(name.to_string(), "Array".to_string());
    }
    if let Type::Map(_, _) = ty {
        builder.var_class.insert(name.to_string(), "Map".to_string());
    }

    // Variable de type Function → enregistrer pour CallIndirect.
    if let Type::Function { ret_ty, .. } = ty {
        builder.func_vars.insert(name.to_string());
        builder.func_ret_types.insert(name.to_string(), IrType::from_ast(ret_ty));
    }

    // Union contenant un type nommé (`Classe|null`) : utiliser le premier
    // `Named` pour l'accès aux champs — voir `union_named_class`.
    if let Some(class_name) = union_named_class(ty) {
        builder.var_class.insert(name.to_string(), class_name);
    }
}

pub fn lower_var(
    builder: &mut LowerBuilder,
    name: &str,
    ty: &Type,
    value: &Expr,
    mutable: bool,
    kind: VarKind,
) {
    let ir_ty = IrType::from_ast(ty);
    
    // Si c'est un tableau, enregistrer le type des éléments
    if let Type::Array(inner) = ty {
        builder.elem_types.insert(name.to_string(), IrType::from_ast(inner));
        builder.elem_ast_types.insert(name.to_string(), (**inner).clone());
    }
    
    // Si c'est une map (ou un union contenant une map, ex. `map<K,V>|null`
    // — voir `map_value_type`), marquer la variable pour Expr::Index →
    // __map_get et enregistrer le type des valeurs dans elem_types
    if let Some(val_ty) = map_value_type(ty) {
        builder.map_vars.insert(name.to_string());
        builder.elem_types.insert(name.to_string(), IrType::from_ast(val_ty));
        builder.elem_ast_types.insert(name.to_string(), val_ty.clone());
    }

    // Classe/générique/string/array/map/function/union : voir register_var_class.
    register_var_class(builder, name, ty);

    let _slot = builder.declare_local(name, ir_ty.clone(), mutable);
    let val_ty = expr_ir_type_pub(builder, value);
    let val = lower_literal_or_expr(builder, value, ty);
    // `value` peut être une `scoped`/`consumed` qui s'échappe vers `name`
    // (point d'échappement — voir crate::lower::stmt::ownership).
    let val = crate::lower::stmt::ownership::maybe_clone_escaping(builder, value, val);
    let val = box_for_any(builder, &ir_ty, val_ty, val);
    
    // Tracker le type de retour original si l'init est un appel async
    // (nécessaire pour l'unboxing dans Expr::Resolve)
    if let Expr::Call { callee, .. } = value {
        if let Expr::Ident(func_name, _) = callee.as_ref() {
            if builder.async_funcs.contains(func_name.as_str()) {
                if let Some(orig_ret) = builder.fn_ret_types.get(func_name.as_str()).cloned() {
                    builder.async_var_ret.insert(name.to_string(), orig_ret);
                }
            }
        }
    }
    
    builder.store_local(name, val);

    // Propriété (`scoped`/`consumed`) — voir crate::lower::stmt::ownership.
    // Après `store_local` : le clonage éventuel à l'échappement (chantier
    // clonage, src/lower/expr.d/lower.rs) a déjà eu lieu en amont dans
    // `val`, ce qui est enregistré ici est bien la copie possédée par CE
    // binding, jamais un alias d'une autre `scoped`/`consumed`.
    crate::lower::stmt::ownership::register_owned_local(builder, name, ty, kind);
}

pub fn lower_const(
    builder: &mut LowerBuilder,
    name: &str,
    ty: &Type,
    value: &Expr,
) {
    let ir_ty = IrType::from_ast(ty);
    
    if let Type::Array(inner) = ty {
        builder.elem_types.insert(name.to_string(), IrType::from_ast(inner));
        builder.elem_ast_types.insert(name.to_string(), (**inner).clone());
    }
    
    if let Some(val_ty) = map_value_type(ty) {
        builder.map_vars.insert(name.to_string());
        builder.elem_types.insert(name.to_string(), IrType::from_ast(val_ty));
        builder.elem_ast_types.insert(name.to_string(), val_ty.clone());
    }

    // Classe/générique/string/array/map/function/union : voir register_var_class.
    // (Avant ce correctif, `lower_const` réimplémentait une copie PARTIELLE
    // de ce bloc — sans les cas Map/Function/Union — voir sa doc pour le
    // bug concret que l'absence du cas Union causait sur `const x:Classe|null`.)
    register_var_class(builder, name, ty);

    let _slot = builder.declare_local(name, ir_ty.clone(), false);
    let val_ty = expr_ir_type_pub(builder, value);
    let val = lower_literal_or_expr(builder, value, ty);
    let val = box_for_any(builder, &ir_ty, val_ty, val);
    builder.store_local(name, val);
}

/// Lower `value` en tenant compte de `ty` (le type DÉCLARÉ de la cible,
/// `var`/`const`) quand `value` est directement un littéral `array`/`map` —
/// permet de choisir `LiteralElemKind::Concrete` (aucune conversion d'un
/// élément `float`/`bool`, stocké brut) plutôt que `Mixed` (boxé) dès que le
/// type d'élément déclaré n'est pas `mixed`. Repli sur `lower_expr` générique
/// (qui suppose toujours `Mixed`, le choix sûr par défaut) dans tous les
/// autres cas — `value` n'est pas directement un littéral, ou son type
/// déclaré n'a pas de type d'élément concret identifiable ici.
fn lower_literal_or_expr(builder: &mut LowerBuilder, value: &Expr, ty: &Type) -> crate::ir::inst::Value {
    use crate::lower::expr::LiteralElemKind;
    // Consommation scalaire directe d'un `message<T>` (générateur — voir
    // docs/roadmap.d/langage-emit-iterable.md, §2) : `var x:T = truc()`/
    // `const x:T = truc()`. Gardé par la sema (au plus un `emit` hors
    // boucle) — voir `crate::sema::typecheck::check_message_scalar_consumption`.
    if let Some((mangled, elem_ty)) = crate::lower::builder::message_gen::detect_message_call(builder, value) {
        return crate::lower::builder::message_gen::lower_message_scalar(builder, value, &mangled, elem_ty);
    }
    match (value, ty) {
        (Expr::Array { elements, .. }, Type::Array(inner)) => {
            let kind = if matches!(inner.as_ref(), Type::Mixed) { LiteralElemKind::Mixed } else { LiteralElemKind::Concrete };
            crate::lower::expr::lower_array_literal(builder, elements, kind)
        }
        (Expr::Map { entries, .. }, Type::Map(_, val_ty)) => {
            let kind = if matches!(val_ty.as_ref(), Type::Mixed) { LiteralElemKind::Mixed } else { LiteralElemKind::Concrete };
            crate::lower::expr::lower_map_literal(builder, entries, kind)
        }
        _ => lower_expr(builder, value),
    }
}

/// Tests unitaires — `map_value_type`
/// (docs/roadmap.d/stdlib-mysql-requetes-parametrees-transactions.md).
#[cfg(test)]
mod tests {
    use super::map_value_type;
    use crate::parsing::ast::Type;

    #[test]
    fn map_value_type_direct_map() {
        let ty = Type::Map(Box::new(Type::String), Box::new(Type::Int));
        assert_eq!(map_value_type(&ty), Some(&Type::Int));
    }

    #[test]
    fn map_value_type_unwraps_union_with_null() {
        // Le cas concret qui a révélé le bug : `MySQL::queryOne` est le seul
        // builtin du langage à retourner `map<string,mixed>|null` — sans ce
        // dépliage, `const one:map<string,mixed>|null = db.queryOne(...)`
        // n'était jamais enregistrée dans `builder.map_vars`, donc
        // `Expr::Index` dispatchait vers `__array_get` au lieu de
        // `__map_get` : `one["champ"]` retournait `null` pour CHAQUE champ,
        // même juste après un narrowing `if one not equal null`. Confirmé
        // par reproduction (`runtime/src/tests/mysql.rs` et test `.oc`
        // manuel) avant ce correctif.
        let ty = Type::Union(vec![
            Type::Map(Box::new(Type::String), Box::new(Type::Mixed)),
            Type::Null,
        ]);
        assert_eq!(map_value_type(&ty), Some(&Type::Mixed));
    }

    #[test]
    fn map_value_type_union_order_independent() {
        // `null` en premier dans l'union doit donner le même résultat —
        // l'ordre des variantes ne doit jamais changer le comportement.
        let ty = Type::Union(vec![Type::Null, Type::Map(Box::new(Type::Int), Box::new(Type::Bool))]);
        assert_eq!(map_value_type(&ty), Some(&Type::Bool));
    }

    #[test]
    fn map_value_type_none_for_non_map_types() {
        assert_eq!(map_value_type(&Type::Int), None);
        assert_eq!(map_value_type(&Type::String), None);
        assert_eq!(map_value_type(&Type::Array(Box::new(Type::Int))), None);
        assert_eq!(map_value_type(&Type::Union(vec![Type::String, Type::Null])), None);
    }
}

/// Tests unitaires — `register_var_class`
/// (docs/roadmap.d/langage-union-class-null-field-access.md) : un `const`/
/// `var` de type `Classe|null` qui retourne réellement une instance (jamais
/// `null`) voyait TOUT accès de champ à l'endroit de l'appel (`f.champ`,
/// même après narrowing `if f is null { return }`) résoudre systématiquement
/// la valeur du PREMIER champ déclaré, quel que soit le champ demandé —
/// `var_class` n'avait aucune entrée pour `f`, donc `Expr::Field`
/// (src/lower/expr.d/lower.rs) retombait sur `offset = 0`.
#[cfg(test)]
mod register_var_class_tests {
    use super::register_var_class;
    use crate::lower::builder::LowerBuilder;
    use crate::ir::module::IrModule;
    use crate::ir::types::IrType;
    use crate::parsing::ast::Type;

    fn builder(module: &mut IrModule) -> LowerBuilder<'_> {
        LowerBuilder::new(module, "test_fn".into(), vec![], IrType::Void)
    }

    /// Cas direct, non-régression : `const x:Foo = ...`/`var x:Foo = ...`
    /// fonctionnait déjà avant ce correctif — doit continuer à fonctionner.
    #[test]
    fn register_var_class_named_class_direct() {
        let mut module = IrModule::new("test");
        let mut b = builder(&mut module);
        register_var_class(&mut b, "x", &Type::Named("Foo".to_string()));
        assert_eq!(b.var_class.get("x"), Some(&"Foo".to_string()));
    }

    /// Le cas exact du bug rapporté : `Foo|null`. Avant ce correctif,
    /// `lower_const` (contrairement à `lower_var`) n'avait aucun cas
    /// `Type::Union` — cette régression aurait échoué en repassant à l'ancien
    /// comportement partiel de `lower_const`.
    #[test]
    fn register_var_class_union_with_null_registers_named_class() {
        let mut module = IrModule::new("test");
        let mut b = builder(&mut module);
        let ty = Type::Union(vec![Type::Named("Foo".to_string()), Type::Null]);
        register_var_class(&mut b, "f", &ty);
        assert_eq!(b.var_class.get("f"), Some(&"Foo".to_string()), "Foo|null doit enregistrer la classe Foo");
    }

    /// `null|Foo` — l'ordre des variantes ne doit pas changer le résultat
    /// (même exigence que `map_value_type_union_order_independent`).
    #[test]
    fn register_var_class_union_order_independent() {
        let mut module = IrModule::new("test");
        let mut b = builder(&mut module);
        let ty = Type::Union(vec![Type::Null, Type::Named("Foo".to_string())]);
        register_var_class(&mut b, "f", &ty);
        assert_eq!(b.var_class.get("f"), Some(&"Foo".to_string()));
    }

    /// `map<K,V>|null`/`Function|...` ne concernent pas `union_named_class`
    /// (aucune variante `Named`) — `register_var_class` ne doit alors rien
    /// enregistrer dans `var_class` pour ce type union (ni planter).
    #[test]
    fn register_var_class_union_without_named_variant_registers_nothing() {
        let mut module = IrModule::new("test");
        let mut b = builder(&mut module);
        let ty = Type::Union(vec![Type::Int, Type::Null]);
        register_var_class(&mut b, "x", &ty);
        assert_eq!(b.var_class.get("x"), None);
    }

    #[test]
    fn register_var_class_string_array_map_builtins() {
        let mut module = IrModule::new("test");
        let mut b = builder(&mut module);
        register_var_class(&mut b, "s", &Type::String);
        register_var_class(&mut b, "a", &Type::Array(Box::new(Type::Int)));
        register_var_class(&mut b, "m", &Type::Map(Box::new(Type::String), Box::new(Type::Int)));
        assert_eq!(b.var_class.get("s"), Some(&"String".to_string()));
        assert_eq!(b.var_class.get("a"), Some(&"Array".to_string()));
        assert_eq!(b.var_class.get("m"), Some(&"Map".to_string()));
    }

    /// Second gap trouvé par la même factorisation : `lower_const` n'avait
    /// jamais eu le cas `Type::Function` (présent dans `lower_var` depuis le
    /// début) — `const f:Function<...> = ...` ne pouvait pas être appelée
    /// par `CallIndirect`.
    #[test]
    fn register_var_class_function_registers_func_vars() {
        let mut module = IrModule::new("test");
        let mut b = builder(&mut module);
        let ty = Type::Function { ret_ty: Box::new(Type::Int), param_tys: vec![] };
        register_var_class(&mut b, "cb", &ty);
        assert!(b.func_vars.contains("cb"));
        assert_eq!(b.func_ret_types.get("cb"), Some(&IrType::I64));
    }

    /// Non-régression : un type primitif ne doit produire AUCUNE entrée.
    #[test]
    fn register_var_class_primitive_registers_nothing() {
        let mut module = IrModule::new("test");
        let mut b = builder(&mut module);
        register_var_class(&mut b, "n", &Type::Int);
        assert_eq!(b.var_class.get("n"), None);
        assert!(!b.func_vars.contains("n"));
    }
}
