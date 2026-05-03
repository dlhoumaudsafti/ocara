/// Inférence de types pour les expressions

use crate::parsing::ast::*;
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;

/// Détermine le type IR d'une expression sans générer de code.
/// Utilisé pour le dispatch typé de `write` et la détection de concat string.
pub fn expr_ir_type(builder: &LowerBuilder, expr: &Expr) -> IrType {
    match expr {
        Expr::Literal(Literal::Int(_), _)    => IrType::I64,
        Expr::Literal(Literal::Float(_), _)  => IrType::F64,
        Expr::Literal(Literal::Bool(_), _)   => IrType::Bool,
        Expr::Literal(Literal::String(_), _) => IrType::Ptr,
        Expr::Literal(Literal::Null, _)      => IrType::Ptr,
        Expr::Ident(name, _) => {
            if let Some((_, ty, _)) = builder.locals.get(name.as_str()) {
                ty.clone()
            } else if let Some((_, _, ty)) = builder.captured_vars.get(name.as_str()) {
                ty.clone()
            } else {
                IrType::Ptr
            }
        }
        // Appels de méthodes String_* et IO_read* retournent des strings
        Expr::StaticCall { class, method, .. } => {
            let fname = format!("{}_{}", class, method);
            // D'abord consulter fn_ret_types (classes locales et builtins enregistrés)
            if let Some(ty) = builder.fn_ret_types.get(&fname) {
                return ty.clone();
            }
            if fname.starts_with("String_")
                || fname.starts_with("IO_read")
                || fname == "__str_concat"
                || fname == "Array_join"
                || fname == "Array_reverse"
                || fname == "Array_slice"
                || fname == "Array_sort"
                || fname.starts_with("Convert_")
                || fname.starts_with("Map_keys")
                || fname.starts_with("Map_values")
                || fname == "System_cwd"
                || fname == "System_exec"
                || fname == "System_env"
                || fname == "HTTPRequest_body"
                || fname == "HTTPRequest_header"
                || fname == "HTTPRequest_error"
            {
                IrType::Ptr
            } else {
                IrType::I64
            }
        }
        // Opérations binaires : propager Ptr si c'est une concat string
        Expr::Binary { op, left, right, .. } => {
            // Comparaisons → Bool
            if matches!(op, BinOp::EqEq | BinOp::NotEq | BinOp::EqEqEq | BinOp::NotEqEq |
                        BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq | BinOp::LtEqEq | BinOp::GtEqEq) {
                return IrType::Bool;
            }
            // Logiques && || → Bool
            if matches!(op, BinOp::And | BinOp::Or) {
                return IrType::Bool;
            }
            if matches!(op, BinOp::Add) {
                let lt = expr_ir_type(builder, left);
                let rt = expr_ir_type(builder, right);
                if matches!(lt, IrType::Ptr) || matches!(rt, IrType::Ptr) {
                    return IrType::Ptr;
                }
                // Float si un des deux est F64
                if matches!(lt, IrType::F64) || matches!(rt, IrType::F64) {
                    return IrType::F64;
                }
            }
            if matches!(op, BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod) {
                let lt = expr_ir_type(builder, left);
                let rt = expr_ir_type(builder, right);
                if matches!(lt, IrType::F64) || matches!(rt, IrType::F64) {
                    return IrType::F64;
                }
            }
            IrType::I64
        }
        // Les templates produisent toujours une string
        Expr::Template { .. } => IrType::Ptr,
        // Match : type déterminé par le premier bras
        Expr::Match { arms, .. } => {
            if let Some(arm) = arms.first() {
                expr_ir_type(builder, &arm.body)
            } else {
                IrType::I64
            }
        }
        // Accès tableau : utilise elem_types si disponible
        Expr::Index { object, .. } => {
            if let Expr::Ident(name, _) = object.as_ref() {
                if let Some(ty) = builder.elem_types.get(name.as_str()) {
                    return ty.clone();
                }
            }
            IrType::I64
        }
        // Accès champ : utilise class_layouts pour connaître le type
        Expr::Field { object, field, .. } => {
            let class_name = match object.as_ref() {
                Expr::Ident(name, _) => builder.var_class.get(name.as_str()).cloned(),
                Expr::SelfExpr(_)    => builder.current_class.clone(),
                _ => None,
            };
            if let Some(cls) = class_name {
                if let Some(fields) = builder.module.class_layouts.get(cls.as_str()) {
                    if let Some((_, ty)) = fields.iter().find(|(f, _)| f == field) {
                        return ty.clone();
                    }
                }
            }
            IrType::Ptr
        }
        // Exception : fonctions utilisateur dont on connaît le type de retour
        Expr::Call { callee, .. } => {
            // Appel indirect : variable de type Function<ReturnType>
            if let Expr::Ident(fname, _) = callee.as_ref() {
                if let Some(ty) = builder.func_ret_types.get(fname.as_str()) {
                    return ty.clone();
                }
            }
            // Callee = Ident (fonction libre)
            if let Expr::Ident(fname, _) = callee.as_ref() {
                if let Some(ty) = builder.fn_ret_types.get(fname.as_str()) {
                    return ty.clone();
                }
            }
            // Callee = méthode obj.method() ou self.method()
            if let Expr::Field { object, field, .. } = callee.as_ref() {
                let class_name = match object.as_ref() {
                    Expr::Ident(name, _) => builder.var_class.get(name.as_str()).cloned(),
                    Expr::SelfExpr(_)    => builder.current_class.clone(),
                    Expr::Literal(Literal::String(_), _) => Some("String".to_string()),
                    // Appel chaîné : obj.method1().method2()
                    Expr::Call { callee: inner_callee, .. } => {
                        if let Expr::Field { object: inner_obj, field: inner_method, .. } = inner_callee.as_ref() {
                            // Essayer de trouver la classe de l'objet interne
                            if let Expr::Ident(name, _) = inner_obj.as_ref() {
                                if let Some(cls) = builder.var_class.get(name.as_str()).cloned() {
                                    let method_name = format!("{}_{}", cls, inner_method);
                                    // Si la méthode retourne un Ptr, continuer avec la même classe
                                    if let Some(ret_ty) = builder.fn_ret_types.get(&method_name) {
                                        if matches!(ret_ty, IrType::Ptr) {
                                            Some(cls)
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(cls) = class_name {
                    let mangled = format!("{}_{}", cls, field);
                    if let Some(ty) = builder.fn_ret_types.get(&mangled) {
                        return ty.clone();
                    }
                }
            }
            IrType::I64
        }
        Expr::StaticConst { class, name, .. } => {
            let key = format!("{}__{}", class, name);
            if let Some((ty, _)) = builder.module.class_consts.get(&key) {
                ty.clone()
            } else {
                IrType::Ptr
            }
        }
        Expr::Unary { op, operand, .. } => {
            match op {
                UnaryOp::Not => IrType::Bool,
                UnaryOp::Neg => expr_ir_type(builder, operand),
            }
        }
        Expr::IsCheck { .. } => IrType::Bool,
        Expr::Resolve { expr, .. } => {
            // Retourne le type IR original de la fonction async sous-jacente.
            match expr.as_ref() {
                Expr::Ident(var_name, _) => {
                    builder.async_var_ret.get(var_name).cloned().unwrap_or(IrType::I64)
                }
                Expr::Call { callee, .. } => {
                    if let Expr::Ident(fn_name, _) = callee.as_ref() {
                        if builder.async_funcs.contains(fn_name.as_str()) {
                            builder.fn_ret_types.get(fn_name.as_str()).cloned().unwrap_or(IrType::I64)
                        } else {
                            IrType::I64
                        }
                    } else {
                        IrType::I64
                    }
                }
                _ => IrType::I64,
            }
        }
        Expr::Nameless { .. } => IrType::Ptr,
        _ => IrType::I64,
    }
}

/// Version publique de expr_ir_type (utilisée par stmt.rs pour le boxing mixed)
pub fn expr_ir_type_pub(builder: &LowerBuilder, expr: &Expr) -> IrType {
    expr_ir_type(builder, expr)
}
