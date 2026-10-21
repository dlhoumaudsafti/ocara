/// Lowering des blocs runtime

use std::rc::Rc;
use std::collections::HashMap;
use crate::parsing::ast::*;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;

/// Transforme les statements dans les blocs runtime (récursivement dans les if/while/etc.)
/// Les `result` sont conservés tels quels et seront transformés lors du lowering
/// (voir statements.rs : result → Store(ERROR) + Jump(runtime_exit_bb))
fn transform_runtime_block_returns(stmts: Vec<Stmt>) -> Vec<Stmt> {
    stmts.into_iter().flat_map(|stmt| {
        transform_runtime_stmt_return(stmt)
    }).collect()
}

fn transform_runtime_stmt_return(stmt: Stmt) -> Vec<Stmt> {
    match stmt {
        Stmt::Result { value, span } => {
            // Les `result` dans les blocs runtime sont maintenant gérés directement
            // par le lowering (voir statements.rs), qui transforme result en
            // Store(ERROR) + Jump(runtime_exit_bb).
            // On garde le statement tel quel dans l'AST.
            vec![Stmt::Result { value, span }]
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
    fn_ret_types: &Rc<HashMap<String, IrType>>,
    fn_param_types: &Rc<HashMap<String, Vec<IrType>>>,
    fn_param_names: &Rc<HashMap<String, Vec<String>>>,
    fn_variadic_info: &Rc<HashMap<String, (usize, IrType)>>,
    func_default_args: &Rc<HashMap<String, Vec<Option<Expr>>>>,
) {
    // Si pas de blocs runtime, pas besoin de générer quoi que ce soit
    if program.runtime_blocks.is_empty() {
        return;
    }

    // Merger tous les statements des blocs dans l'ordre : init → main → success → exit
    generate_runtime_main(module, program, consts, fn_ret_types, fn_param_types, fn_param_names, fn_variadic_info, func_default_args);
}

/// Génère la fonction main() avec tous les statements des blocs runtime mergés
fn generate_runtime_main(
    module: &mut IrModule,
    program: &Program,
    consts: &[ConstDecl],
    fn_ret_types: &Rc<HashMap<String, IrType>>,
    fn_param_types: &Rc<HashMap<String, Vec<IrType>>>,
    fn_param_names: &Rc<HashMap<String, Vec<String>>>,
    fn_variadic_info: &Rc<HashMap<String, (usize, IrType)>>,
    func_default_args: &Rc<HashMap<String, Vec<Option<Expr>>>>,
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
        kind: crate::parsing::ast::VarKind::Var,
        span: Span::new(0, 0),
    });
    
    // var SUCCESS:bool = false (sera mis à true si tout va bien)
    all_stmts.push(crate::parsing::ast::Stmt::Var {
        name: "SUCCESS".to_string(),
        ty: crate::parsing::ast::Type::Bool,
        value: crate::parsing::ast::Expr::Literal(crate::parsing::ast::Literal::Bool(false), Span::new(0, 0)),
        mutable: true,
        kind: crate::parsing::ast::VarKind::Var,
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
                op: crate::parsing::ast::BinOp::NotEqual,
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
        fn_param_names,
        fn_variadic_info,
        func_default_args,
    );
}

/// Slots pointeur des déclarations de premier niveau d'`init`/`main`, créés
/// à 0 avant toute instruction : un `result` qui saute une déclaration laisse
/// 0 dans son slot, libéré sans effet à la fin (voir `runtime_preslots`).
fn preset_runtime_slots(builder: &mut crate::lower::builder::LowerBuilder, stmts: &[crate::parsing::ast::Stmt]) {
    use crate::ir::inst::Inst;
    use crate::parsing::ast::Stmt;
    for stmt in stmts {
        let (Stmt::Var { name, ty, .. } | Stmt::Const { name, ty, .. }) = stmt else { continue };
        if IrType::from_ast(ty) != IrType::Ptr {
            continue;
        }
        let slot = builder.new_value();
        builder.emit(Inst::Alloca { dest: slot.clone(), ty: IrType::Ptr });
        let zero = builder.new_value();
        builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
        builder.emit(Inst::Store { ptr: slot.clone(), src: zero });
        builder.runtime_preslots.insert(name.clone(), slot);
    }
}

/// Lower manuel de la fonction main des blocs runtime pour gérer le label runtime_exit_bb
///
/// `ERROR`/`SUCCESS` sont lowered hors de toute portée (lisibles jusqu'au
/// `return ERROR` final). Tout le reste — `init`, `main`, le `if` des blocs
/// `error`/`success`, `exit` — partage UNE portée (`enter_block` /
/// `leave_block`) : les variables d'`init`/`main` restent vivantes jusqu'à la
/// fin de `exit` (EBNF §5.3), et les `scoped`/`consumed` y sont détruites
/// (`block_scope_stack`, comme un bloc ordinaire). Un `result` de `main` ne
/// libère que les portées plus internes avant de sauter au label ; une
/// déclaration de premier niveau qu'il saute garde un slot à 0
/// (`preset_runtime_slots`). Auparavant `init`+`main` formaient un bloc
/// libéré avant `error`/`success`/`exit` : objet lu dans `exit` → SIGSEGV,
/// chaîne → `null` (voir docs/roadmap.d/langage-runtime-blocs-variables.md).
fn lower_runtime_main_manual(
    module: &mut IrModule,
    all_stmts: Vec<crate::parsing::ast::Stmt>,
    main_end_index: usize,
    consts: &[ConstDecl],
    fn_ret_types: &Rc<HashMap<String, IrType>>,
    fn_param_types: &Rc<HashMap<String, Vec<IrType>>>,
    fn_param_names: &Rc<HashMap<String, Vec<String>>>,
    fn_variadic_info: &Rc<HashMap<String, (usize, IrType)>>,
    func_default_args: &Rc<HashMap<String, Vec<Option<Expr>>>>,
) {
    use crate::lower::builder::LowerBuilder;
    use crate::lower::stmt::statements::lower_stmt;
    use crate::lower::stmt::block::{enter_block, leave_block, lower_block_stmts};
    use crate::ir::inst::Inst;
    use crate::parsing::ast::Block;
    use crate::parsing::token::Span;

    let mut builder = LowerBuilder::new(module, "main".to_string(), vec![], IrType::I64);
    builder.fn_ret_types = fn_ret_types.clone();
    builder.fn_param_types = fn_param_types.clone();
    builder.fn_param_names = fn_param_names.clone();
    builder.fn_variadic_info = fn_variadic_info.clone();
    builder.func_default_args = func_default_args.clone();
    // Pas de portée comptée dans ce `main` : constantes jamais relâchées.
    let _ = super::functions::lower_const_prologue(&mut builder, consts, true);

    // Créer le label de sortie anticipée (avant le if ERROR != 0)
    let runtime_exit_label = builder.new_block();
    builder.runtime_exit_bb = Some(runtime_exit_label.clone());

    // `ERROR`/`SUCCESS` (les 2 premiers statements, voir `generate_runtime_main`) :
    // variables "de fonction" qui doivent rester lisibles/modifiables dans
    // TOUTES les phases suivantes (if error/success, exit, `return ERROR`
    // final) — lowered directement, PAS via `lower_block`, qui supprimerait
    // leur slot de `builder.locals` en sortie de bloc (voir sa doc).
    const RUNTIME_VARS_COUNT: usize = 2;
    for stmt in &all_stmts[..RUNTIME_VARS_COUNT] {
        lower_stmt(&mut builder, stmt);
    }

    // init + main + error/success + exit : UNE portée (voir la doc de cette
    // fonction) — les variables déclarées dans `init`/`main` restent
    // vivantes jusqu'à la fin de `exit` (EBNF §5.3), libérées au `return
    // ERROR` final.
    let scope_block = Block {
        stmts: all_stmts[RUNTIME_VARS_COUNT..all_stmts.len() - 1].to_vec(),
        span: Span::new(0, 0),
    };
    let state = enter_block(&mut builder);
    builder.runtime_scope = Some((builder.block_scope_stack.len() - 1, builder.rc_scopes.len() - 1));
    preset_runtime_slots(&mut builder, &all_stmts[RUNTIME_VARS_COUNT..main_end_index]);
    lower_block_stmts(&mut builder, &all_stmts[RUNTIME_VARS_COUNT..main_end_index]);

    // Basculer vers le label de sortie anticipée
    if !builder.is_terminated() {
        builder.emit(Inst::Jump { target: runtime_exit_label.clone() });
    }
    builder.switch_to(&runtime_exit_label);

    // Désactiver runtime_exit_bb pour que les returns suivants ne soient plus transformés
    builder.runtime_exit_bb = None;

    // Le if (ERROR != 0) { error } else { success }, puis `exit` ; le
    // `return ERROR` final (toujours le dernier statement, voir
    // `generate_runtime_main`) libère toute la portée.
    lower_block_stmts(&mut builder, &all_stmts[main_end_index..all_stmts.len() - 1]);
    lower_stmt(&mut builder, &all_stmts[all_stmts.len() - 1]);
    builder.runtime_scope = None;
    builder.runtime_preslots.clear();
    leave_block(&mut builder, state, &scope_block);

    if !builder.is_terminated() {
        let zero = builder.new_value();
        builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
        builder.emit(Inst::Return { value: Some(zero) });
    }

    let func = builder.func;
    module.add_function(func);
}

