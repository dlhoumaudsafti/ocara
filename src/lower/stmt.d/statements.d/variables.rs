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

    // Si c'est un type de classe, enregistrer le mapping var → classe
    if let Type::Named(class_name) = ty {
        builder.var_class.insert(name.to_string(), class_name.clone());
    }
    
    // Si c'est un générique, utiliser le nom monomorphisé
    if let Type::Generic { name: generic_name, args } = ty {
        let specialized_name = monomorphized_name(generic_name, args);
        builder.var_class.insert(name.to_string(), specialized_name);
    }
    
    // Les variables string ont automatiquement accès aux méthodes de String
    if let Type::String = ty {
        builder.var_class.insert(name.to_string(), "String".to_string());
    }
    
    // Les variables array ont automatiquement accès aux méthodes de Array
    if let Type::Array(_) = ty {
        builder.var_class.insert(name.to_string(), "Array".to_string());
    }
    
    // Les variables map ont automatiquement accès aux méthodes de Map
    if let Type::Map(_, _) = ty {
        builder.var_class.insert(name.to_string(), "Map".to_string());
    }
    
    // Variable de type Function → enregistrer pour CallIndirect
    if let Type::Function { ret_ty, .. } = ty {
        builder.func_vars.insert(name.to_string());
        builder.func_ret_types.insert(name.to_string(), IrType::from_ast(ret_ty));
    }
    
    // Union contenant un type nommé : utiliser le premier Named pour l'accès aux champs
    if let Type::Union(variants) = ty {
        if let Some(class_name) = variants.iter().find_map(|v| {
            if let Type::Named(n) = v { Some(n.clone()) } else { None }
        }) {
            builder.var_class.insert(name.to_string(), class_name);
        }
    }
    
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

    if let Type::Named(class_name) = ty {
        builder.var_class.insert(name.to_string(), class_name.clone());
    }

    // Si c'est un générique, utiliser le nom monomorphisé
    if let Type::Generic { name: generic_name, args } = ty {
        let specialized_name = monomorphized_name(generic_name, args);
        builder.var_class.insert(name.to_string(), specialized_name);
    }
    
    // Les variables string ont automatiquement accès aux méthodes de String
    if let Type::String = ty {
        builder.var_class.insert(name.to_string(), "String".to_string());
    }
    
    // Les variables array ont automatiquement accès aux méthodes de Array
    if let Type::Array(_) = ty {
        builder.var_class.insert(name.to_string(), "Array".to_string());
    }
    
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
