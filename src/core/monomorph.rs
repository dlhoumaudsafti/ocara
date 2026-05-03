use crate::parsing::ast::{self, ClassDecl, ClassMember, Expr, FuncDecl, Stmt, Type};
use std::collections::{HashMap, HashSet};

/// Génère un nom unique pour une classe monomorphisée
pub fn monomorphized_name(base: &str, type_args: &[ast::Type]) -> String {
    let mut name = base.to_string();
    for ty in type_args {
        name.push('_');
        name.push_str(&type_name_for_mangle(ty));
    }
    name
}

/// Convertit un type en chaîne pour le name mangling
fn type_name_for_mangle(ty: &Type) -> String {
    match ty {
        Type::Int => "int".to_string(),
        Type::Float => "float".to_string(),
        Type::Bool => "bool".to_string(),
        Type::String => "string".to_string(),
        Type::Void => "void".to_string(),
        Type::Mixed => "mixed".to_string(),
        Type::Null => "null".to_string(),
        Type::Named(n) => n.clone(),
        Type::Qualified(parts) => parts.join("_"),
        Type::Array(inner) => format!("array_{}", type_name_for_mangle(inner)),
        Type::Map(k, v) => format!("map_{}_{}", type_name_for_mangle(k), type_name_for_mangle(v)),
        Type::Generic { name, args } => {
            let mut s = name.clone();
            for arg in args {
                s.push('_');
                s.push_str(&type_name_for_mangle(arg));
            }
            s
        }
        Type::Union(variants) => {
            let mut s = String::from("union");
            for v in variants {
                s.push('_');
                s.push_str(&type_name_for_mangle(v));
            }
            s
        }
        Type::Function { .. } => "function".to_string(),
    }
}

/// Substitue les paramètres de type dans un Type
fn substitute_type(ty: &Type, type_params: &[String], type_args: &[Type]) -> Type {
    match ty {
        Type::Named(n) => {
            // Chercher si c'est un paramètre de type
            if let Some(idx) = type_params.iter().position(|p| p == n) {
                if idx < type_args.len() {
                    return type_args[idx].clone();
                }
            }
            ty.clone()
        }
        Type::Array(inner) => {
            Type::Array(Box::new(substitute_type(inner, type_params, type_args)))
        }
        Type::Map(k, v) => {
            Type::Map(
                Box::new(substitute_type(k, type_params, type_args)),
                Box::new(substitute_type(v, type_params, type_args)),
            )
        }
        Type::Generic { name, args } => {
            // Substituer récursivement dans les arguments
            let new_args: Vec<Type> = args
                .iter()
                .map(|arg| substitute_type(arg, type_params, type_args))
                .collect();
            Type::Generic { name: name.clone(), args: new_args }
        }
        Type::Union(variants) => {
            Type::Union(
                variants
                    .iter()
                    .map(|v| substitute_type(v, type_params, type_args))
                    .collect()
            )
        }
        Type::Function { ret_ty, param_tys } => {
            Type::Function {
                ret_ty: Box::new(substitute_type(ret_ty, type_params, type_args)),
                param_tys: param_tys
                    .iter()
                    .map(|p| substitute_type(p, type_params, type_args))
                    .collect(),
            }
        }
        _ => ty.clone(),
    }
}

/// Substitue les paramètres de type dans une expression
fn substitute_expr(expr: &mut Expr, type_params: &[String], type_args: &[Type], mapping: &HashMap<(String, Vec<Type>), String>) {
    match expr {
        Expr::New { class, type_args: ta, args, .. } => {
            // Si c'est un générique, remplacer par le nom monomorphisé
            if !ta.is_empty() {
                if let Some(specialized_name) = mapping.get(&(class.clone(), ta.clone())) {
                    *class = specialized_name.clone();
                    ta.clear();
                }
            }
            for arg in args {
                substitute_expr(arg, type_params, type_args, mapping);
            }
        }
        Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(..) | Expr::StaticConst { .. } => {}
        Expr::Binary { left, right, .. } => {
            substitute_expr(left, type_params, type_args, mapping);
            substitute_expr(right, type_params, type_args, mapping);
        }
        Expr::Unary { operand, .. } => {
            substitute_expr(operand, type_params, type_args, mapping);
        }
        Expr::Call { callee, args, .. } => {
            substitute_expr(callee, type_params, type_args, mapping);
            for arg in args {
                substitute_expr(arg, type_params, type_args, mapping);
            }
        }
        Expr::StaticCall { args, .. } => {
            for arg in args {
                substitute_expr(arg, type_params, type_args, mapping);
            }
        }
        Expr::Field { object, .. } => {
            substitute_expr(object, type_params, type_args, mapping);
        }
        Expr::Index { object, index, .. } => {
            substitute_expr(object, type_params, type_args, mapping);
            substitute_expr(index, type_params, type_args, mapping);
        }
        Expr::Array { elements, .. } => {
            for elem in elements {
                substitute_expr(elem, type_params, type_args, mapping);
            }
        }
        Expr::Map { entries, .. } => {
            for (k, v) in entries {
                substitute_expr(k, type_params, type_args, mapping);
                substitute_expr(v, type_params, type_args, mapping);
            }
        }
        Expr::Range { start, end, .. } => {
            substitute_expr(start, type_params, type_args, mapping);
            substitute_expr(end, type_params, type_args, mapping);
        }
        Expr::Match { subject, arms, .. } => {
            substitute_expr(subject, type_params, type_args, mapping);
            for arm in arms {
                substitute_expr(&mut arm.body, type_params, type_args, mapping);
            }
        }
        Expr::Template { parts, .. } => {
            for part in parts {
                if let ast::TemplatePartExpr::Expr(e) = part {
                    substitute_expr(e, type_params, type_args, mapping);
                }
            }
        }
        Expr::Nameless { body, .. } => {
            for stmt in &mut body.stmts {
                substitute_stmt(stmt, type_params, type_args, mapping);
            }
        }
        Expr::Resolve { expr: e, .. } => {
            substitute_expr(e, type_params, type_args, mapping);
        }
        Expr::IsCheck { expr: e, .. } => {
            substitute_expr(e, type_params, type_args, mapping);
        }
    }
}

/// Substitue les paramètres de type dans un statement
fn substitute_stmt(stmt: &mut Stmt, type_params: &[String], type_args: &[Type], mapping: &HashMap<(String, Vec<Type>), String>) {
    match stmt {
        Stmt::Var { ty, value, .. } => {
            *ty = substitute_type(ty, type_params, type_args);
            substitute_expr(value, type_params, type_args, mapping);
        }
        Stmt::Const { ty, value, .. } => {
            *ty = substitute_type(ty, type_params, type_args);
            substitute_expr(value, type_params, type_args, mapping);
        }
        Stmt::Return { value, .. } => {
            if let Some(e) = value {
                substitute_expr(e, type_params, type_args, mapping);
            }
        }
        Stmt::If { condition, then_block, elseif, else_block, .. } => {
            substitute_expr(condition, type_params, type_args, mapping);
            for s in &mut then_block.stmts {
                substitute_stmt(s, type_params, type_args, mapping);
            }
            for (cond, block) in elseif {
                substitute_expr(cond, type_params, type_args, mapping);
                for s in &mut block.stmts {
                    substitute_stmt(s, type_params, type_args, mapping);
                }
            }
            if let Some(els) = else_block {
                for s in &mut els.stmts {
                    substitute_stmt(s, type_params, type_args, mapping);
                }
            }
        }
        Stmt::While { condition, body, .. } => {
            substitute_expr(condition, type_params, type_args, mapping);
            for s in &mut body.stmts {
                substitute_stmt(s, type_params, type_args, mapping);
            }
        }
        Stmt::ForIn { iter, body, .. } | Stmt::ForMap { iter, body, .. } => {
            substitute_expr(iter, type_params, type_args, mapping);
            for s in &mut body.stmts {
                substitute_stmt(s, type_params, type_args, mapping);
            }
        }
        Stmt::Switch { subject, cases, default, .. } => {
            substitute_expr(subject, type_params, type_args, mapping);
            for case in cases {
                for s in &mut case.body.stmts {
                    substitute_stmt(s, type_params, type_args, mapping);
                }
            }
            if let Some(def) = default {
                for s in &mut def.stmts {
                    substitute_stmt(s, type_params, type_args, mapping);
                }
            }
        }
        Stmt::Try { body, handlers, .. } => {
            for s in &mut body.stmts {
                substitute_stmt(s, type_params, type_args, mapping);
            }
            for handler in handlers {
                for s in &mut handler.body.stmts {
                    substitute_stmt(s, type_params, type_args, mapping);
                }
            }
        }
        Stmt::Expr(expr) => {
            substitute_expr(expr, type_params, type_args, mapping);
        }
        Stmt::Raise { value, .. } => {
            substitute_expr(value, type_params, type_args, mapping);
        }
        Stmt::Assign { target, value, .. } => {
            substitute_expr(target, type_params, type_args, mapping);
            substitute_expr(value, type_params, type_args, mapping);
        }
        Stmt::Break { .. } | Stmt::Continue { .. } => {}
    }
}

/// Collecte tous les usages de génériques dans le programme
fn collect_generic_instantiations(program: &ast::Program) -> HashSet<(String, Vec<Type>)> {
    let mut instantiations = HashSet::new();
    
    // Parcourir toutes les fonctions
    for func in &program.functions {
        collect_from_func(func, &mut instantiations);
    }
    
    // Parcourir toutes les classes
    for class in &program.classes {
        for member in &class.members {
            if let ClassMember::Method { decl, .. } = member {
                collect_from_func(decl, &mut instantiations);
            } else if let ClassMember::Constructor { body, .. } = member {
                for stmt in &body.stmts {
                    collect_from_stmt(stmt, &mut instantiations);
                }
            }
        }
    }
    
    instantiations
}

fn collect_from_func(func: &FuncDecl, instantiations: &mut HashSet<(String, Vec<Type>)>) {
    for stmt in &func.body.stmts {
        collect_from_stmt(stmt, instantiations);
    }
}

fn collect_from_stmt(stmt: &Stmt, instantiations: &mut HashSet<(String, Vec<Type>)>) {
    match stmt {
        Stmt::Var { value, .. } | Stmt::Const { value, .. } => {
            collect_from_expr(value, instantiations);
        }
        Stmt::Return { value, .. } => {
            if let Some(e) = value {
                collect_from_expr(e, instantiations);
            }
        }
        Stmt::If { condition, then_block, elseif, else_block, .. } => {
            collect_from_expr(condition, instantiations);
            for s in &then_block.stmts {
                collect_from_stmt(s, instantiations);
            }
            for (cond, block) in elseif {
                collect_from_expr(cond, instantiations);
                for s in &block.stmts {
                    collect_from_stmt(s, instantiations);
                }
            }
            if let Some(els) = else_block {
                for s in &els.stmts {
                    collect_from_stmt(s, instantiations);
                }
            }
        }
        Stmt::While { condition, body, .. } => {
            collect_from_expr(condition, instantiations);
            for s in &body.stmts {
                collect_from_stmt(s, instantiations);
            }
        }
        Stmt::ForIn { iter, body, .. } | Stmt::ForMap { iter, body, .. } => {
            collect_from_expr(iter, instantiations);
            for s in &body.stmts {
                collect_from_stmt(s, instantiations);
            }
        }
        Stmt::Switch { subject, cases, default, .. } => {
            collect_from_expr(subject, instantiations);
            for case in cases {
                for s in &case.body.stmts {
                    collect_from_stmt(s, instantiations);
                }
            }
            if let Some(def) = default {
                for s in &def.stmts {
                    collect_from_stmt(s, instantiations);
                }
            }
        }
        Stmt::Try { body, handlers, .. } => {
            for s in &body.stmts {
                collect_from_stmt(s, instantiations);
            }
            for handler in handlers {
                for s in &handler.body.stmts {
                    collect_from_stmt(s, instantiations);
                }
            }
        }
        Stmt::Expr(expr) => {
            collect_from_expr(expr, instantiations);
        }
        Stmt::Raise { value, .. } => {
            collect_from_expr(value, instantiations);
        }
        Stmt::Assign { target, value, .. } => {
            collect_from_expr(target, instantiations);
            collect_from_expr(value, instantiations);
        }
        Stmt::Break { .. } | Stmt::Continue { .. } => {}
    }
}

fn collect_from_expr(expr: &Expr, instantiations: &mut HashSet<(String, Vec<Type>)>) {
    match expr {
        Expr::New { class, type_args, args, .. } => {
            if !type_args.is_empty() {
                instantiations.insert((class.clone(), type_args.clone()));
            }
            for arg in args {
                collect_from_expr(arg, instantiations);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_from_expr(left, instantiations);
            collect_from_expr(right, instantiations);
        }
        Expr::Unary { operand, .. } => {
            collect_from_expr(operand, instantiations);
        }
        Expr::Call { callee, args, .. } => {
            collect_from_expr(callee, instantiations);
            for arg in args {
                collect_from_expr(arg, instantiations);
            }
        }
        Expr::StaticCall { args, .. } => {
            for arg in args {
                collect_from_expr(arg, instantiations);
            }
        }
        Expr::Field { object, .. } => {
            collect_from_expr(object, instantiations);
        }
        Expr::Index { object, index, .. } => {
            collect_from_expr(object, instantiations);
            collect_from_expr(index, instantiations);
        }
        Expr::Array { elements, .. } => {
            for elem in elements {
                collect_from_expr(elem, instantiations);
            }
        }
        Expr::Map { entries, .. } => {
            for (k, v) in entries {
                collect_from_expr(k, instantiations);
                collect_from_expr(v, instantiations);
            }
        }
        Expr::Range { start, end, .. } => {
            collect_from_expr(start, instantiations);
            collect_from_expr(end, instantiations);
        }
        Expr::Match { subject, arms, .. } => {
            collect_from_expr(subject, instantiations);
            for arm in arms {
                collect_from_expr(&arm.body, instantiations);
            }
        }
        Expr::Template { parts, .. } => {
            for part in parts {
                if let ast::TemplatePartExpr::Expr(e) = part {
                    collect_from_expr(e, instantiations);
                }
            }
        }
        Expr::Nameless { body, .. } => {
            for stmt in &body.stmts {
                collect_from_stmt(stmt, instantiations);
            }
        }
        Expr::Resolve { expr: e, .. } => {
            collect_from_expr(e, instantiations);
        }
        Expr::IsCheck { expr: e, .. } => {
            collect_from_expr(e, instantiations);
        }
        Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(..) | Expr::StaticConst { .. } => {}
    }
}

/// Monomorphise les génériques : génère des classes spécialisées
pub fn monomorphize(program: &mut ast::Program) {
    // 1. Collecter tous les usages de génériques
    let instantiations = collect_generic_instantiations(program);
    
    if instantiations.is_empty() {
        return;
    }
    
    // 2. Créer un mapping (generic_name, type_args) -> specialized_name
    let mut mapping: HashMap<(String, Vec<Type>), String> = HashMap::new();
    let mut specialized_classes: Vec<ClassDecl> = Vec::new();
    
    for (generic_name, type_args) in &instantiations {
        // Trouver la déclaration générique
        let generic_decl = match program.generics.iter().find(|g| &g.name == generic_name) {
            Some(g) => g,
            None => continue, // Générique non trouvé (erreur déjà signalée par le typecheck)
        };
        
        // Générer le nom spécialisé
        let specialized_name = monomorphized_name(generic_name, type_args);
        mapping.insert((generic_name.clone(), type_args.clone()), specialized_name.clone());
        
        // Extraire les noms des paramètres de type
        let type_param_names: Vec<String> = generic_decl.type_params.iter().map(|tp| tp.name.clone()).collect();
        
        // Créer une ClassDecl spécialisée
        let mut specialized_members: Vec<ClassMember> = Vec::new();
        
        for member in &generic_decl.members {
            let specialized_member = match member {
                ClassMember::Field { vis, mutable, name, ty, span } => {
                    ClassMember::Field {
                        vis: vis.clone(),
                        mutable: *mutable,
                        name: name.clone(),
                        ty: substitute_type(ty, &type_param_names, type_args),
                        span: span.clone(),
                    }
                }
                ClassMember::Method { vis, is_static, decl, span } => {
                    let mut specialized_decl = decl.clone();
                    // Substituer les types dans les paramètres
                    for param in &mut specialized_decl.params {
                        param.ty = substitute_type(&param.ty, &type_param_names, type_args);
                    }
                    // Substituer le type de retour
                    specialized_decl.ret_ty = substitute_type(&specialized_decl.ret_ty, &type_param_names, type_args);
                    // Substituer dans le corps
                    for stmt in &mut specialized_decl.body.stmts {
                        substitute_stmt(stmt, &type_param_names, type_args, &mapping);
                    }
                    ClassMember::Method {
                        vis: vis.clone(),
                        is_static: *is_static,
                        decl: specialized_decl,
                        span: span.clone(),
                    }
                }
                ClassMember::Constructor { params, body, span } => {
                    let mut specialized_params = params.clone();
                    for param in &mut specialized_params {
                        param.ty = substitute_type(&param.ty, &type_param_names, type_args);
                    }
                    let mut specialized_body = body.clone();
                    for stmt in &mut specialized_body.stmts {
                        substitute_stmt(stmt, &type_param_names, type_args, &mapping);
                    }
                    ClassMember::Constructor {
                        params: specialized_params,
                        body: specialized_body,
                        span: span.clone(),
                    }
                }
                ClassMember::Const { vis, name, ty, value, span } => {
                    ClassMember::Const {
                        vis: vis.clone(),
                        name: name.clone(),
                        ty: substitute_type(ty, &type_param_names, type_args),
                        value: value.clone(),
                        span: span.clone(),
                    }
                }
            };
            specialized_members.push(specialized_member);
        }
        
        let specialized_class = ClassDecl {
            name: specialized_name,
            extends: generic_decl.extends.clone(),
            modules: generic_decl.modules.clone(),
            implements: generic_decl.implements.clone(),
            members: specialized_members,
            span: generic_decl.span.clone(),
        };
        
        specialized_classes.push(specialized_class);
    }
    
    // 3. Ajouter les classes spécialisées au programme
    program.classes.extend(specialized_classes);
    
    // 4. Remplacer tous les usages de Type::Generic par Type::Named dans le programme
    for func in &mut program.functions {
        for stmt in &mut func.body.stmts {
            substitute_stmt(stmt, &[], &[], &mapping);
        }
    }
    
    for class in &mut program.classes {
        for member in &mut class.members {
            match member {
                ClassMember::Method { decl, .. } => {
                    for stmt in &mut decl.body.stmts {
                        substitute_stmt(stmt, &[], &[], &mapping);
                    }
                }
                ClassMember::Constructor { body, .. } => {
                    for stmt in &mut body.stmts {
                        substitute_stmt(stmt, &[], &[], &mapping);
                    }
                }
                _ => {}
            }
        }
    }
}
