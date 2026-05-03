/// Helpers pour le lowering des expressions

use std::collections::HashMap;
use crate::parsing::ast::Expr;
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;
use crate::codegen::runtime::builtins;

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
            matches!(
                format!("{}_{}", class, method).as_str(),
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
