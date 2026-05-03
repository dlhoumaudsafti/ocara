/// Lowering des classes

use std::collections::{HashMap, HashSet};
use crate::parsing::ast::*;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use super::functions::lower_func;

pub fn lower_class(
    module: &mut IrModule,
    class: &ClassDecl,
    all_classes: &[ClassDecl],
    all_modules: &[ModuleDecl],
    consts: &[crate::parsing::ast::ConstDecl],
    fn_ret_types: &HashMap<String, IrType>,
    fn_param_types: &HashMap<String, Vec<IrType>>,
    fn_variadic_info: &HashMap<String, (usize, IrType)>,
    func_default_args: &HashMap<String, Vec<Option<Expr>>>,
    async_funcs: &HashSet<String>,
) {
    // Collecte des noms de méthodes propres (pour détecter les surcharges)
    let own_methods: HashSet<String> = class.members.iter()
        .filter_map(|m| if let ClassMember::Method { decl, .. } = m { Some(decl.name.clone()) } else { None })
        .collect();

    for member in &class.members {
        match member {
            ClassMember::Method { decl, is_static, .. } => {
                if *is_static {
                    // Méthode statique : pas de self, appelée via Class::method()
                    let mangled = FuncDecl {
                        name: format!("{}_{}", class.name, decl.name),
                        ..decl.clone()
                    };
                    lower_func(module, &mangled, consts, fn_ret_types, fn_param_types, fn_variadic_info, func_default_args, None, async_funcs);
                } else {
                    // Méthode d'instance : self en premier paramètre
                    let self_param = crate::parsing::ast::Param {
                        name: "self".into(),
                        ty:   Type::Mixed,
                        is_variadic: false,
                        default_value: None,
                        span: decl.span.clone(),
                    };
                    let mut full_params = vec![self_param];
                    full_params.extend(decl.params.clone());
                    let mangled = FuncDecl {
                        name:   format!("{}_{}", class.name, decl.name),
                        params: full_params,
                        ..decl.clone()
                    };
                    lower_func(module, &mangled, consts, fn_ret_types, fn_param_types, fn_variadic_info, func_default_args, Some(&class.name), async_funcs);
                }
            }
            ClassMember::Constructor { params, body, span } => {
                let self_param = crate::parsing::ast::Param {
                    name: "self".into(),
                    ty:   Type::Mixed,
                    is_variadic: false,
                    default_value: None,
                    span: span.clone(),
                };
                let mut full_params = vec![self_param];
                full_params.extend(params.clone());
                let init_func = FuncDecl {
                    name:     format!("{}_init", class.name),
                    params:   full_params,
                    ret_ty:   Type::Void,
                    body:     body.clone(),
                    is_async: false,
                    span:     span.clone(),
                };
                lower_func(module, &init_func, consts, fn_ret_types, fn_param_types, fn_variadic_info, func_default_args, Some(&class.name), async_funcs);
            }
            ClassMember::Const { name, value, .. } => {
                use crate::ir::module::IrGlobal;
                let bytes = match value {
                    Expr::Literal(Literal::Int(n), _)    => n.to_le_bytes().to_vec(),
                    Expr::Literal(Literal::Float(f), _)  => f.to_le_bytes().to_vec(),
                    Expr::Literal(Literal::Bool(b), _)   => vec![*b as u8],
                    Expr::Literal(Literal::String(s), _) => s.as_bytes().to_vec(),
                    _ => vec![],
                };
                module.add_global(IrGlobal {
                    name: format!("{}__{}" , class.name, name),
                    bytes,
                });
            }
            ClassMember::Field { .. } => {}
        }
    }

    // Émettre les méthodes des modules pour cette classe
    for module_name in &class.modules {
        if let Some(module_decl) = all_modules.iter().find(|m| &m.name == module_name) {
            for member in &module_decl.members {
                if let ClassMember::Method { decl, is_static, .. } = member {
                    // Ne générer que si la classe n'a pas surchargé cette méthode
                    if !own_methods.contains(&decl.name) {
                        if *is_static {
                            // Méthode statique du module (rare mais possible)
                            let mangled = FuncDecl {
                                name: format!("{}_{}", class.name, decl.name),
                                ..decl.clone()
                            };
                            lower_func(module, &mangled, consts, fn_ret_types, fn_param_types, fn_variadic_info, func_default_args, None, async_funcs);
                        } else {
                            // Méthode d'instance du module
                            let self_param = crate::parsing::ast::Param {
                                name: "self".into(),
                                ty:   Type::Mixed,
                                is_variadic: false,
                                default_value: None,
                                span: decl.span.clone(),
                            };
                            let mut full_params = vec![self_param];
                            full_params.extend(decl.params.clone());
                            let mangled = FuncDecl {
                                name:   format!("{}_{}", class.name, decl.name),
                                params: full_params,
                                ..decl.clone()
                            };
                            // Générer avec le contexte de la classe (layouts corrects!)
                            lower_func(module, &mangled, consts, fn_ret_types, fn_param_types, fn_variadic_info, func_default_args, Some(&class.name), async_funcs);
                        }
                    }
                }
            }
        }
    }

    // Émettre les méthodes héritées non surchargées comme Child_method
    if let Some(parent_name) = &class.extends {
        if let Some(parent) = all_classes.iter().find(|c| c.name == *parent_name) {
            for member in &parent.members {
                if let ClassMember::Method { decl, .. } = member {
                    if !own_methods.contains(&decl.name) {
                        let self_param = crate::parsing::ast::Param {
                            name: "self".into(),
                            ty:   Type::Mixed,
                            is_variadic: false,
                            default_value: None,
                            span: decl.span.clone(),
                        };
                        let mut full_params = vec![self_param];
                        full_params.extend(decl.params.clone());
                        let mangled = FuncDecl {
                            name:   format!("{}_{}", class.name, decl.name),
                            params: full_params,
                            ..decl.clone()
                        };
                        // Émettre avec le contexte de la classe enfant (layouts corrects)
                        lower_func(module, &mangled, consts, fn_ret_types, fn_param_types, fn_variadic_info, func_default_args, Some(&class.name), async_funcs);
                    }
                }
            }
        }
    }
}
