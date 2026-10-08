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
    fn_param_names: &HashMap<String, Vec<String>>,
    fn_variadic_info: &HashMap<String, (usize, IrType)>,
    func_default_args: &HashMap<String, Vec<Option<Expr>>>,
    class_name: Option<&str>,
    parent_class: Option<&str>,
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
    // `main(): void` retourne quand même un code de sortie : 0.
    let ret_ty = match IrType::from_ast(&func.ret_ty) {
        IrType::Void if func.name == "main" => IrType::I64,
        ty => ty,
    };

    let mut builder = LowerBuilder::new(module, func.name.clone(), ir_params.clone(), ret_ty);
    builder.ret_ast_ty = Some(func.ret_ty.clone());
    // Si on est dans une méthode/constructeur de classe, enregistrer la classe courante
    if let Some(cls) = class_name {
        builder.current_class = Some(cls.to_string());
        builder.var_class.insert("self".to_string(), cls.to_string());
    }
    // Si la classe a un parent, l'enregistrer aussi
    if let Some(parent) = parent_class {
        builder.parent_class = Some(parent.to_string());
    }
    builder.fn_ret_types = fn_ret_types.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    builder.fn_param_types = fn_param_types.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    builder.fn_param_names = fn_param_names.iter()
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
        
        // Paramètre variadic : `param.ty` EST le type d'élément (`variadic<bool>`
        // → `bool`), jamais un `Type::Array` — sans cette entrée, `for f in
        // flags` laissait `f` boxé (`not f`/`if f` testaient le pointeur de la
        // cellule, toujours « vrai »). Voir docs/roadmap.d/langage-variadic-bool-not.md.
        if param.is_variadic {
            builder.elem_types.insert(param.name.clone(), IrType::from_ast(&param.ty));
            builder.elem_ast_types.insert(param.name.clone(), param.ty.clone());
        }

        // Si le paramètre est un tableau, enregistrer le type d'élément pour
        // les boucles for — jamais pour un variadic, dont `param.ty` est le
        // type d'ÉLÉMENT (`variadic<array<int>>` : `ty` = `array<int>`, les
        // éléments sont des tableaux, pas des `int`), déjà enregistré plus haut.
        if let (false, crate::parsing::ast::Type::Array(inner)) = (param.is_variadic, &param.ty) {
            let elem_ty = IrType::from_ast(inner);
            builder.elem_types.insert(param.name.clone(), elem_ty);
            builder.elem_ast_types.insert(param.name.clone(), (**inner).clone());
        }
        
        // Marquer les paramètres de type map<> pour Expr::Index → __map_get
        if let (false, crate::parsing::ast::Type::Map(_, val_ty)) = (param.is_variadic, &param.ty) {
            builder.map_vars.insert(param.name.clone());
            builder.elem_types.insert(param.name.clone(), IrType::from_ast(val_ty));
            builder.elem_ast_types.insert(param.name.clone(), (**val_ty).clone());
        }
        // Marquer les paramètres de type Function pour CallIndirect
        if let crate::parsing::ast::Type::Function { ret_ty, .. } = &param.ty {
            builder.func_vars.insert(param.name.clone());
            builder.func_ret_types.insert(param.name.clone(), IrType::from_ast(ret_ty));
        }
        // Classe du paramètre — même résolution qu'une variable locale
        // (classe, générique, union `Classe|null`, et `String`/`Array`/`Map`
        // pour les builtins : sans ça, `xs.push(...)` sur un paramètre
        // `array<T>` était compilé en `String_push`, SIGSEGV).
        crate::lower::stmt::statements::register_var_class(&mut builder, &param.name, &param.ty);
        // Un paramètre de type générique (`Box<int>`) : même résolution que
        // pour une variable locale directement initialisée (voir
        // `lower_var`/`lower_const` dans statements.d/variables.rs) — sans
        // ça, `var_class` n'a aucune entrée pour ce paramètre et tout appel
        // de méthode dessus dans le corps de la fonction/méthode retombe sur
        // le fallback `_method_<nom>` (jamais généré) au lieu de
        // `Box_int_<nom>` : confirmé par reproduction (voir
        // docs/roadmap.d/langage-generiques.md).
        if let crate::parsing::ast::Type::Generic { name: generic_name, args } = &param.ty {
            let specialized_name = crate::core::monomorph::monomorphized_name(generic_name, args);
            builder.var_class.insert(param.name.clone(), specialized_name);
        }
        // Un paramètre `Classe|null` (le pattern officiellement documenté
        // pour un retour "peut échouer", ex. `function find(id:int): User|null`,
        // EBNF.md) doit aussi être enregistré dans var_class — sans ce
        // dépliage, un accès de champ (`Expr::Field`) sur le paramètre après
        // narrowing (`if u is null { return }`) ne trouve aucune classe et
        // retombe sur l'offset 0 pour n'importe quel champ. Même bug/même
        // correctif que `register_var_class`/`union_named_class`
        // (src/lower/stmt.d/statements.d/variables.rs) — voir
        // docs/roadmap.d/langage-union-class-null-field-access.md.
        if let Some(class_name) = crate::parsing::ast::union_named_class(&param.ty) {
            builder.var_class.insert(param.name.clone(), class_name);
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
    crate::lower::stmt::rc::begin_function(&mut builder, &func.params);
    crate::lower::stmt::lower_block(&mut builder, &func.body);
    crate::lower::stmt::rc::end_function(&mut builder);

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
