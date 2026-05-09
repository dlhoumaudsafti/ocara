/// Lowering des blocs runtime

use std::collections::{HashMap, HashSet};
use crate::parsing::ast::*;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;

/// Transforme les statements dans les blocs runtime (récursivement dans les if/while/etc.)
/// Les returns sont conservés tels quels et seront transformés lors du lowering
/// (voir statements.rs : return → Store(ERROR) + Jump(runtime_exit_bb))
fn transform_runtime_block_returns(stmts: Vec<Stmt>) -> Vec<Stmt> {
    stmts.into_iter().flat_map(|stmt| {
        transform_runtime_stmt_return(stmt)
    }).collect()
}

fn transform_runtime_stmt_return(stmt: Stmt) -> Vec<Stmt> {
    match stmt {
        Stmt::Return { value, span } => {
            // Les returns dans les blocs runtime sont maintenant gérés directement
            // par le lowering (voir statements.rs), qui transforme return en
            // Store(ERROR) + Jump(runtime_exit_bb).
            // On garde le return tel quel dans l'AST.
            vec![Stmt::Return { value, span }]
        }
        
        // Transformer récursivement dans les blocs imbriqués
        Stmt::If { condition, then_block, elseif, else_block, span } => {
            vec![Stmt::If {
                condition,
                then_block: Block {
                    stmts: transform_runtime_block_returns(then_block.stmts),
                    span: then_block.span,
                },
                elseif: elseif.into_iter().map(|(cond, block)| {
                    (cond, Block {
                        stmts: transform_runtime_block_returns(block.stmts),
                        span: block.span,
                    })
                }).collect(),
                else_block: else_block.map(|block| Block {
                    stmts: transform_runtime_block_returns(block.stmts),
                    span: block.span,
                }),
                span,
            }]
        }
        
        Stmt::While { condition, body, span } => {
            vec![Stmt::While {
                condition,
                body: Block {
                    stmts: transform_runtime_block_returns(body.stmts),
                    span: body.span,
                },
                span,
            }]
        }
        
        Stmt::ForIn { var, iter, body, span } => {
            vec![Stmt::ForIn {
                var,
                iter,
                body: Block {
                    stmts: transform_runtime_block_returns(body.stmts),
                    span: body.span,
                },
                span,
            }]
        }
        
        Stmt::ForMap { key, value, iter, body, span } => {
            vec![Stmt::ForMap {
                key,
                value,
                iter,
                body: Block {
                    stmts: transform_runtime_block_returns(body.stmts),
                    span: body.span,
                },
                span,
            }]
        }
        
        Stmt::Try { body, handlers, span } => {
            vec![Stmt::Try {
                body: Block {
                    stmts: transform_runtime_block_returns(body.stmts),
                    span: body.span,
                },
                // NE PAS transformer les returns dans les handlers !
                // Ils doivent être gérés par le système de propagation de return
                // des handlers d'exceptions (voir exceptions.rs)
                handlers,
                span,
            }]
        }
        
        Stmt::Switch { subject, cases, default, span } => {
            vec![Stmt::Switch {
                subject,
                cases: cases.into_iter().map(|case| SwitchCase {
                    pattern: case.pattern,
                    body: Block {
                        stmts: transform_runtime_block_returns(case.body.stmts),
                        span: case.body.span,
                    },
                    span: case.span,
                }).collect(),
                default: default.map(|block| Block {
                    stmts: transform_runtime_block_returns(block.stmts),
                    span: block.span,
                }),
                span,
            }]
        }
        
        // Les autres statements ne contiennent pas de returns
        _ => vec![stmt],
    }
}

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
    let has_success = program.runtime_blocks.iter().any(|b| b.kind == crate::parsing::ast::RuntimeBlockKind::Success);
    
    // Injecter les variables magiques ERROR et SUCCESS au début
    // Elles sont toujours déclarées pour permettre leur utilisation dans tous les blocs
    // (par exemple: return ERROR dans le bloc main)
    // var ERROR:int = 0
    all_stmts.push(crate::parsing::ast::Stmt::Var {
        name: "ERROR".to_string(),
        ty: crate::parsing::ast::Type::Int,
        value: crate::parsing::ast::Expr::Literal(crate::parsing::ast::Literal::Int(0), Span::new(0, 0)),
        mutable: true,
        span: Span::new(0, 0),
    });
    
    // var SUCCESS:bool = false (sera mis à true si tout va bien)
    all_stmts.push(crate::parsing::ast::Stmt::Var {
        name: "SUCCESS".to_string(),
        ty: crate::parsing::ast::Type::Bool,
        value: crate::parsing::ast::Expr::Literal(crate::parsing::ast::Literal::Bool(false), Span::new(0, 0)),
        mutable: true,
        span: Span::new(0, 0),
    });
    
    // Ajouter les blocs init et main (sans enveloppe conditionnelle)
    // Les returns seront transformés en assignations ERROR + checks
    let mut main_end_index = 2; // Après ERROR et SUCCESS
    for kind in &[crate::parsing::ast::RuntimeBlockKind::Init, crate::parsing::ast::RuntimeBlockKind::Main] {
        if let Some(block) = program.runtime_blocks.iter().find(|b| b.kind == *kind) {
            // Transformer les returns dans les statements du bloc
            let transformed_stmts = transform_runtime_block_returns(block.statements.clone());
            all_stmts.extend(transformed_stmts.clone());
            main_end_index += transformed_stmts.len();
        }
    }
    
    // Ajouter la logique conditionnelle pour error/success
    
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
    
    // Lower manuellement la fonction main pour insérer le label runtime_exit_bb
    lower_runtime_main_manual(
        module,
        all_stmts,
        main_end_index,
        consts,
        fn_ret_types,
        fn_param_types,
        fn_variadic_info,
        func_default_args,
        async_funcs,
    );
}

/// Lower manuel de la fonction main des blocs runtime pour gérer le label runtime_exit_bb
fn lower_runtime_main_manual(
    module: &mut IrModule,
    all_stmts: Vec<crate::parsing::ast::Stmt>,
    main_end_index: usize,
    _consts: &[ConstDecl],
    fn_ret_types: &HashMap<String, IrType>,
    fn_param_types: &HashMap<String, Vec<IrType>>,
    fn_variadic_info: &HashMap<String, (usize, IrType)>,
    func_default_args: &HashMap<String, Vec<Option<Expr>>>,
    async_funcs: &HashSet<String>,
) {
    use crate::lower::builder::LowerBuilder;
    use crate::lower::stmt::statements::lower_stmt;
    use crate::ir::inst::Inst;
    
    let mut builder = LowerBuilder::new(module, "main".to_string(), vec![], IrType::I64);
    builder.fn_ret_types = fn_ret_types.clone();
    builder.fn_param_types = fn_param_types.clone();
    builder.fn_variadic_info = fn_variadic_info.clone();
    builder.func_default_args = func_default_args.clone();
    builder.async_funcs = async_funcs.clone();
    
    // Créer le label de sortie anticipée (avant le if ERROR != 0)
    let runtime_exit_label = builder.new_block();
    builder.runtime_exit_bb = Some(runtime_exit_label.clone());
    
    // Le split_point est maintenant main_end_index, c'est-à-dire après les blocs init et main
    let split_point = main_end_index;
    
    // Lower tous les statements AVANT le if (ERROR != 0)
    for stmt in &all_stmts[..split_point] {
        // Si le bloc est déjà terminé, ne pas lower les statements suivants (dead code)
        if builder.is_terminated() {
            break;
        }
        lower_stmt(&mut builder, stmt);
    }
    
    
    // Basculer vers le label de sortie anticipée
    if !builder.is_terminated() {
        builder.emit(Inst::Jump { target: runtime_exit_label.clone() });
    }
    builder.switch_to(&runtime_exit_label);
    
    // Désactiver runtime_exit_bb pour que les returns suivants ne soient plus transformés
    builder.runtime_exit_bb = None;
    
    // Lower le if (ERROR != 0), exit, et return
    for stmt in &all_stmts[split_point..] {
        lower_stmt(&mut builder, stmt);
    }
    
    if !builder.is_terminated() {
        let zero = builder.new_value();
        builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
        builder.emit(Inst::Return { value: Some(zero) });
    }
    
    let func = builder.func;
    module.add_function(func);
}

