/// Helpers pour le lowering des expressions

use std::collections::HashMap;
use crate::parsing::ast::{Expr, Literal, Type};
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;
use crate::codegen::runtime::builtins;

/// Résout le nom de classe d'un accès de champ CHAÎNÉ (`w.inner` où `inner`
/// est elle-même une instance de classe/`string`/`array`/`map`) — récursif
/// pour gérer plus de deux niveaux (`a.b.c.d`).
///
/// Bug historique corrigé par cette fonction : chaque site qui résout la
/// classe d'un `object` pour un accès de champ/appel de méthode
/// (`Expr::Field`/`Expr::Call` avec callee `Field`) ne savait gérer que
/// `object` = `Ident`/`SelfExpr`/`ParentExpr`/littéral string — jamais
/// `object` = un AUTRE `Expr::Field`. `w.inner.sum()` (où `inner:Point`)
/// tombait alors dans le fallback générique de chaque site ("le type IR
/// est Ptr, je suppose que c'est une String"), qui devinait la MAUVAISE
/// classe silencieusement (`String_sum` au lieu de `Point_sum`) — pas
/// d'erreur de compilation, juste une valeur incorrecte au runtime.
///
/// Nécessite `IrModule.class_field_types` (vrai `Type` AST par champ — pas
/// `IrType`, qui réduit classe/string/array/map à `Ptr`, tous
/// indistinguables) : ne résout donc que les champs de classes
/// UTILISATEUR (voir sa doc) — un champ d'une classe builtin/opaque
/// retourne `None` ici, comme avant ce correctif.
pub fn resolve_chained_field_class(builder: &LowerBuilder, object: &Expr, field: &str) -> Option<String> {
    let base_class = match object {
        Expr::Ident(name, _)    => builder.var_class.get(name.as_str()).cloned(),
        Expr::SelfExpr(_)       => builder.current_class.clone(),
        Expr::ParentExpr(_)     => builder.parent_class.clone(),
        Expr::Literal(Literal::String(_), _) => Some("String".to_string()),
        Expr::Field { object: inner, field: inner_field, .. } => {
            resolve_chained_field_class(builder, inner, inner_field)
        }
        _ => None,
    }?;
    let field_ty = builder.module.class_field_types.get(&base_class)?
        .iter().find(|(f, _)| f == field)
        .map(|(_, ty)| ty.clone())?;
    match field_ty {
        Type::Named(n)   => Some(n),
        Type::String     => Some("String".to_string()),
        Type::Array(_)   => Some("Array".to_string()),
        Type::Map(_, _)  => Some("Map".to_string()),
        _ => None,
    }
}

/// Complète les arguments avec les valeurs par défaut si nécessaire
pub fn complete_args_with_defaults(
    builder: &LowerBuilder,
    func_name: &str,
    args: &[Expr],
) -> Vec<Expr> {
    // Récupérer les valeurs par défaut de la fonction
    let default_args = match builder.func_default_args.get(func_name) {
        Some(defaults) => defaults,
        None => return args.to_vec(), // Pas de valeurs par défaut
    };
    
    // Si tous les arguments sont fournis, retourner tel quel
    if args.len() >= default_args.len() {
        return args.to_vec();
    }
    
    // Compléter avec les valeurs par défaut manquantes
    let mut completed = args.to_vec();
    for i in args.len()..default_args.len() {
        if let Some(ref default_expr) = default_args[i] {
            completed.push(default_expr.clone());
        }
    }
    
    completed
}

/// Calcule l'offset en bytes d'un champ dans une classe (8 bytes par champ).
pub fn field_offset(layouts: &HashMap<String, Vec<(String, IrType)>>, class: &str, field: &str) -> i32 {
    if let Some(fields) = layouts.get(class) {
        if let Some(idx) = fields.iter().position(|(f, _)| f == field) {
            return (idx as i32) * 8;
        }
    }
    0
}

/// Retourne le type IR d'un champ depuis le class_layout.
pub fn field_ir_type(layouts: &HashMap<String, Vec<(String, IrType)>>, class: &str, field: &str) -> IrType {
    if let Some(fields) = layouts.get(class) {
        if let Some((_, ty)) = fields.iter().find(|(f, _)| f == field) {
            return ty.clone();
        }
    }
    IrType::Ptr
}

/// Retourne true si l'expression produit un tableau (OcaraArray*).
/// Utilisé dans les templates pour appeler __array_to_str au lieu de ptr_to_str.
pub fn is_array_expr(builder: &LowerBuilder, expr: &Expr) -> bool {
    match expr {
        Expr::Ident(name, _) => builder.elem_types.contains_key(name.as_str()),
        Expr::StaticCall { class, method, .. } => {
            // Résoudre "<parent>" et "<self>" vers les classes appropriées
            let resolved_class = if class == "<parent>" {
                builder.parent_class.as_deref().unwrap_or(class.as_str())
            } else if class == "<self>" {
                builder.current_class.as_deref().unwrap_or(class.as_str())
            } else {
                class.as_str()
            };
            matches!(
                format!("{}_{}", resolved_class, method).as_str(),
                "System_args"
                | "Array_sort"
                | "Array_reverse"
                | "Array_slice"
                | "Map_keys_to_array"
                | "Map_values_to_array"
            )
        }
        _ => false,
    }
}

/// Retourne true si une fonction builtin retourne void (returns: None dans builtins()).
pub fn is_void_builtin(func_name: &str) -> bool {
    builtins().iter().any(|b| b.name == func_name && b.returns.is_none())
}

/// Retourne le nom de la variante typée de `write` selon le type de l'argument.
/// "write"       → string/mixed (pas de conversion, write(ptr) direct)
/// "write_int"   → entiers
/// "write_float" → flottants (prend f64)
/// "write_bool"  → booléens
pub fn write_variant(base: &str, ty: &IrType) -> String {
    let suffix = match ty {
        IrType::F64  => "Float",
        IrType::Bool => "Bool",
        IrType::I64  => "Int",
        _            => "",   // Ptr / Mixed → write directement
    };
    format!("{}{}", base, suffix)
}
