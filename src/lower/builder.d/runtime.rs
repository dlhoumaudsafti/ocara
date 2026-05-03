/// Lowering des blocs runtime

use std::collections::{HashMap, HashSet};
use crate::parsing::ast::*;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use super::functions::lower_func;

pub fn lower_runtime_blocks(
    module: &mut IrModule,
    program: &Program,
    consts: &[ConstDecl],
    fn_ret_types: &HashMap<String, IrType>,
    fn_param_types: &HashMap<String, Vec<IrType>>,
    fn_variadic_info: &HashMap<String, (usize, IrType)>,
    func_default_args: &HashMap<String, Vec<Option<Expr>>>,
    async_funcs: &HashSet<String>,
) {
    // Si pas de blocs runtime, pas besoin de générer quoi que ce soit
    if program.runtime_blocks.is_empty() {
        return;
    }
    
    // Merger tous les statements des blocs dans l'ordre : init → main → success → exit
    generate_runtime_main(module, program, consts, fn_ret_types, fn_param_types, fn_variadic_info, func_default_args, async_funcs);
}

/// Génère la fonction main() avec tous les statements des blocs runtime mergés
fn generate_runtime_main(
    module: &mut IrModule,
    program: &Program,
    consts: &[ConstDecl],
    fn_ret_types: &HashMap<String, IrType>,
    fn_param_types: &HashMap<String, Vec<IrType>>,
    fn_variadic_info: &HashMap<String, (usize, IrType)>,
    func_default_args: &HashMap<String, Vec<Option<Expr>>>,
    async_funcs: &HashSet<String>,
) {
    use crate::parsing::token::Span;
    
    // Collecter les statements de chaque bloc dans l'ordre
    let mut all_stmts = vec![];
    
    // Vérifier quels blocs existent
    let has_error = program.runtime_blocks.iter().any(|b| b.kind == crate::parsing::ast::RuntimeBlockKind::Error);
    let has_exit = program.runtime_blocks.iter().any(|b| b.kind == crate::parsing::ast::RuntimeBlockKind::Exit);
    
    // Injecter les variables magiques au début si nécessaire
    if has_error || has_exit {
        // var ERROR:int = 0
        all_stmts.push(crate::parsing::ast::Stmt::Var {
            name: "ERROR".to_string(),
            ty: crate::parsing::ast::Type::Int,
            value: crate::parsing::ast::Expr::Literal(crate::parsing::ast::Literal::Int(0), Span::new(0, 0)),
            mutable: true,
            span: Span::new(0, 0),
        });
    }
    
    if has_exit {
        // var SUCCESS:bool = false (sera mis à true si tout va bien)
        all_stmts.push(crate::parsing::ast::Stmt::Var {
            name: "SUCCESS".to_string(),
            ty: crate::parsing::ast::Type::Bool,
            value: crate::parsing::ast::Expr::Literal(crate::parsing::ast::Literal::Bool(false), Span::new(0, 0)),
            mutable: true,
            span: Span::new(0, 0),
        });
    }
    
    // Ajouter les blocs init et main
    for kind in &[crate::parsing::ast::RuntimeBlockKind::Init, crate::parsing::ast::RuntimeBlockKind::Main] {
        if let Some(block) = program.runtime_blocks.iter().find(|b| b.kind == *kind) {
            all_stmts.extend(block.statements.clone());
        }
    }
    
    // Ajouter la logique conditionnelle pour error/success
    let has_success = program.runtime_blocks.iter().any(|b| b.kind == crate::parsing::ast::RuntimeBlockKind::Success);
    
    if has_error || has_success {
        // Créer le bloc if (error != 0) { error } else { success = true; success }
        let mut if_body = vec![];  // Corps du bloc error
        let mut else_body = vec![]; // Corps du bloc success
        
        // Si le bloc error existe, ajouter ses statements
        if let Some(error_block) = program.runtime_blocks.iter().find(|b| b.kind == crate::parsing::ast::RuntimeBlockKind::Error) {
            if_body.extend(error_block.statements.clone());
        }
        
        // Dans le else, mettre SUCCESS = true puis les statements du bloc success
        if has_success {
            else_body.push(crate::parsing::ast::Stmt::Assign {
                target: crate::parsing::ast::Expr::Ident("SUCCESS".to_string(), Span::new(0, 0)),
                value: crate::parsing::ast::Expr::Literal(crate::parsing::ast::Literal::Bool(true), Span::new(0, 0)),
                span: Span::new(0, 0),
            });
            
            if let Some(success_block) = program.runtime_blocks.iter().find(|b| b.kind == crate::parsing::ast::RuntimeBlockKind::Success) {
                else_body.extend(success_block.statements.clone());
            }
        }
        
        // Créer le if statement : if (ERROR != 0)
        all_stmts.push(crate::parsing::ast::Stmt::If {
            condition: crate::parsing::ast::Expr::Binary {
                left: Box::new(crate::parsing::ast::Expr::Ident("ERROR".to_string(), Span::new(0, 0))),
                op: crate::parsing::ast::BinOp::NotEq,
                right: Box::new(crate::parsing::ast::Expr::Literal(crate::parsing::ast::Literal::Int(0), Span::new(0, 0))),
                span: Span::new(0, 0),
            },
            then_block: crate::parsing::ast::Block {
                stmts: if_body,
                span: Span::new(0, 0),
            },
            elseif: vec![],
            else_block: if !else_body.is_empty() {
                Some(crate::parsing::ast::Block {
                    stmts: else_body,
                    span: Span::new(0, 0),
                })
            } else {
                None
            },
            span: Span::new(0, 0),
        });
    }
    
    // Ajouter le bloc exit
    if let Some(exit_block) = program.runtime_blocks.iter().find(|b| b.kind == crate::parsing::ast::RuntimeBlockKind::Exit) {
        all_stmts.extend(exit_block.statements.clone());
    }
    
    // Retourner la valeur de ERROR (0 si succès, autre si erreur)
    all_stmts.push(crate::parsing::ast::Stmt::Return {
        value: Some(crate::parsing::ast::Expr::Ident("ERROR".to_string(), Span::new(0, 0))),
        span: Span::new(0, 0),
    });
    
    // Créer la fonction main() AST avec tous les statements mergés
    let main_func = crate::parsing::ast::FuncDecl {
        name: "main".to_string(),
        params: vec![],
        ret_ty: crate::parsing::ast::Type::Int,
        body: crate::parsing::ast::Block {
            stmts: all_stmts,
            span: Span::new(0, 0),
        },
        span: Span::new(0, 0),
        is_async: false,
    };
    
    // Lower la fonction
    lower_func(
        module,
        &main_func,
        consts,
        fn_ret_types,
        fn_param_types,
        fn_variadic_info,
        func_default_args,
        None,
        async_funcs,
    );
}

