/// Lowering des déclarations de variables et constantes

use crate::parsing::ast::*;
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;
use crate::lower::expr::{lower_expr, expr_ir_type_pub};
use crate::core::monomorph::monomorphized_name;
use super::helpers::box_for_any;

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
    
    // Si c'est une map, marquer la variable pour Expr::Index → __map_get
    // et enregistrer le type des valeurs dans elem_types
    if let Type::Map(_, val_ty) = ty {
        builder.map_vars.insert(name.to_string());
        builder.elem_types.insert(name.to_string(), IrType::from_ast(val_ty));
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
    let val = lower_expr(builder, value);
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
    
    if let Type::Map(_, val_ty) = ty {
        builder.map_vars.insert(name.to_string());
        builder.elem_types.insert(name.to_string(), IrType::from_ast(val_ty));
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
    let val = lower_expr(builder, value);
    let val = box_for_any(builder, &ir_ty, val_ty, val);
    builder.store_local(name, val);
}
