/// Lowering des fonctions et constantes globales

use std::collections::{HashMap, HashSet};
use crate::parsing::ast::*;
use crate::ir::func::IrParam;
use crate::ir::inst::{Inst, Value};
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use super::types::LowerBuilder;

pub fn lower_const_global(module: &mut IrModule, c: &ConstDecl) {
    use crate::ir::module::IrGlobal;

    let bytes = match &c.value {
        Expr::Literal(Literal::Int(n), _)   => n.to_le_bytes().to_vec(),
        Expr::Literal(Literal::Float(f), _) => f.to_le_bytes().to_vec(),
        Expr::Literal(Literal::Bool(b), _)  => vec![*b as u8],
        Expr::Literal(Literal::String(s), _) => s.as_bytes().to_vec(),
        Expr::Literal(Literal::Null, _)      => vec![0u8; 8],
        _ => vec![],
    };
    module.add_global(IrGlobal { name: c.name.clone(), bytes });
}

// ─────────────────────────────────────────────────────────────────────────────
// Fonction libre
// ─────────────────────────────────────────────────────────────────────────────

pub fn lower_func(
    module: &mut IrModule,
    func: &FuncDecl,
    consts: &[crate::parsing::ast::ConstDecl],
    fn_ret_types: &HashMap<String, IrType>,
    fn_param_types: &HashMap<String, Vec<IrType>>,
    fn_variadic_info: &HashMap<String, (usize, IrType)>,
    func_default_args: &HashMap<String, Vec<Option<Expr>>>,
    class_name: Option<&str>,
    async_funcs: &HashSet<String>,
) {
    // Transformer les paramètres : si variadic, le dernier devient Ptr (tableau)
    let ir_params: Vec<IrParam> = func.params.iter().enumerate().map(|(i, p)| {
        let ty = if p.is_variadic {
            // Le paramètre variadic devient un pointeur vers tableau
            IrType::Ptr
        } else {
            IrType::from_ast(&p.ty)
        };
        IrParam {
            name: p.name.clone(),
            ty,
            slot: Value(i as u32),
        }
    }).collect();
    let ret_ty = IrType::from_ast(&func.ret_ty);

    let mut builder = LowerBuilder::new(module, func.name.clone(), ir_params.clone(), ret_ty);
    // Si on est dans une méthode/constructeur de classe, enregistrer la classe courante
    if let Some(cls) = class_name {
        builder.current_class = Some(cls.to_string());
        builder.var_class.insert("self".to_string(), cls.to_string());
    }
    builder.fn_ret_types = fn_ret_types.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    builder.fn_param_types = fn_param_types.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    builder.fn_variadic_info = fn_variadic_info.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    builder.func_default_args = func_default_args.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    builder.async_funcs = async_funcs.iter().cloned().collect();
    for c in consts {
        let ir_ty = IrType::from_ast(&c.ty);
        let _slot = builder.declare_local(&c.name, ir_ty, false);
        let val = crate::lower::expr::lower_expr(&mut builder, &c.value);
        builder.store_local(&c.name, val);
    }

    // Enregistre les paramètres comme locaux immuables et met à jour IrParam::slot
    // pour pointer vers l'alloca réel (les consts ont avancé next_value).
    let updated_params: Vec<IrParam> = func.params.iter().enumerate().map(|(idx, param)| {
        // Déterminer le type IR : si variadic, c'est Ptr (déjà géré plus haut)
        let ir_ty = ir_params[idx].ty.clone();
        
        // Si le paramètre est variadic, le marquer
        if param.is_variadic {
            builder.variadic_params.insert(param.name.clone());
        }
        
        // Si le paramètre est un tableau (y compris variadic désucré en T[]),
        // enregistrer le type d'élément pour les boucles for
        if let crate::parsing::ast::Type::Array(inner) = &param.ty {
            let elem_ty = IrType::from_ast(inner);
            builder.elem_types.insert(param.name.clone(), elem_ty);
        }
        
        // Marquer les paramètres de type map<> pour Expr::Index → __map_get
        if let crate::parsing::ast::Type::Map(_, _) = &param.ty {
            builder.map_vars.insert(param.name.clone());
        }
        // Marquer les paramètres de type Function pour CallIndirect
        if let crate::parsing::ast::Type::Function { ret_ty, .. } = &param.ty {
            builder.func_vars.insert(param.name.clone());
            builder.func_ret_types.insert(param.name.clone(), IrType::from_ast(ret_ty));
        }
        // Slot alloca qui recevra la valeur du paramètre
        let alloca_slot = builder.declare_local(&param.name, ir_ty.clone(), false);
        // Variable « receiver » distincte : mappée aux block_params Cranelift
        let receiver = builder.new_value();
        // Store initial : param_receiver → alloca_slot (remplit le slot stack)
        builder.emit(Inst::Store { ptr: alloca_slot, src: receiver.clone() });
        // IrParam::slot pointe vers le receiver (utilisé dans le block-param mapping)
        IrParam { name: param.name.clone(), ty: ir_ty, slot: receiver }
    }).collect();
    builder.func.params = updated_params;

    // Body
    crate::lower::stmt::lower_block(&mut builder, &func.body);

    // Return implicite si le bloc courant n'est pas terminé
    if !builder.is_terminated() {
        let ret_ty_copy = builder.func.ret_ty.clone();
        let ret_val = if ret_ty_copy != IrType::Void {
            // Bloc mort (toutes les branches ont retourné) : valeur dummy
            let zero = builder.new_value();
            builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
            Some(zero)
        } else {
            None
        };
        builder.emit(Inst::Return { value: ret_val });
    }

    let ir_func = builder.func;
    module.add_function(ir_func);
}
