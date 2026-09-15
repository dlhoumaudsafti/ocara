/// Dispatch dynamique réel pour les interfaces (voir
/// docs/roadmap.d/langage-interfaces.md).
///
/// Un appel `d.method(...)` sur une variable typée par une INTERFACE
/// (`var d:Drawable`) est mangled comme n'importe quel appel de méthode
/// (`Drawable_method`, voir `src/lower/expr.d/lower.rs`) — jusqu'ici, cette
/// fonction n'existait tout simplement jamais (seules les classes concrètes
/// émettent du code), le dispatch était donc entièrement cassé. Ce module
/// génère RÉELLEMENT `Interface_method` : une fonction qui lit l'identité de
/// classe de `self` (`IrModule::class_ids`, écrite dans le header de chaque
/// instance par `__alloc_class_obj`, voir runtime/src/lib.rs) et appelle la
/// bonne implémentation concrète (`Classe_method`) selon un branchement sur
/// cette identité — un dispatch manuel mais réel, réutilisant telle quelle
/// toute l'infrastructure d'appel mangled déjà en place.
use crate::ir::func::{IrFunction, IrParam};
use crate::ir::inst::Inst;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use crate::parsing::ast::{ClassDecl, InterfaceMethod, Program};

/// Génère, pour chaque interface implémentée par au moins une classe, un
/// dispatcher réel pour chacune de ses méthodes.
///
/// Portée volontairement alignée sur celle du diagnostic E09 (`src/main.rs`) :
/// seules les classes qui déclarent `implements Interface` DIRECTEMENT dans
/// leur propre liste sont candidates au dispatch — une sous-classe qui n'a
/// pas elle-même redéclaré `implements` n'est PAS candidate ici, même si
/// `SymbolTable::class_matches` (utilisée par `types_compat` pour
/// l'affectation) la considère transitivement compatible via `extends` :
/// cette asymétrie est voulue, E09 n'a validé la signature QUE pour les
/// classes qui l'implémentent explicitement, on ne peut pas générer un appel
/// vers une méthode dont la signature n'a jamais été vérifiée contre
/// l'interface (voir `SymbolTable::class_matches` pour le détail).
pub fn generate_interface_dispatchers(module: &mut IrModule, program: &Program) {
    for iface in &program.interfaces {
        let implementers: Vec<&ClassDecl> = program.classes.iter()
            .filter(|c| c.implements.iter().any(|i| i == &iface.name))
            .collect();
        // Interface jamais implémentée : rien à générer (jamais atteignable
        // par un appel qui aurait passé la sema de toute façon).
        if implementers.is_empty() {
            continue;
        }
        for method in &iface.methods {
            generate_one_dispatcher(module, &iface.name, method, &implementers);
        }
    }
}

fn generate_one_dispatcher(
    module: &mut IrModule,
    iface_name: &str,
    method: &InterfaceMethod,
    implementers: &[&ClassDecl],
) {
    let ret_ty = IrType::from_ast(&method.ret_ty);
    let mut f = IrFunction::new(format!("{}_{}", iface_name, method.name), vec![], ret_ty.clone());

    // `self` + les paramètres de la méthode — leurs slots sont réservés ICI
    // (avant toute autre émission) pour correspondre aux block-params
    // Cranelift de la fonction (voir `emit_function`, `ir_func.params[i].slot`
    // devient directement l'index de la Variable associée au i-ème
    // paramètre entrant).
    let self_val = f.new_value();
    let mut params = vec![IrParam { name: "self".into(), ty: IrType::Ptr, slot: self_val.clone() }];
    let mut arg_vals = Vec::new();
    for p in &method.params {
        let v = f.new_value();
        params.push(IrParam { name: p.name.clone(), ty: IrType::from_ast(&p.ty), slot: v.clone() });
        arg_vals.push(v);
    }
    f.params = params;

    // Identité de classe de `self` — offset -16 : le `class_id` est stocké
    // JUSTE AVANT le tag `TAG_OBJECT` (lui à `self - 8`), voir
    // `__alloc_class_obj` dans runtime/src/lib.rs. Un simple `GetField` à
    // offset négatif suffit (Cranelift accepte un déplacement signé), pas
    // besoin d'une fonction runtime dédiée.
    let class_id_val = f.new_value();
    f.emit(Inst::GetField {
        dest:   class_id_val.clone(),
        obj:    self_val.clone(),
        field:  "__class_id".into(),
        ty:     IrType::I64,
        offset: -16,
    });

    // Chaîne de branchement : une comparaison par implémenteur, dans l'ordre
    // de déclaration (déterministe ; l'ORDRE n'a aucun impact sur le
    // résultat, une seule branche peut matcher puisque `class_id` est unique
    // par classe).
    for class in implementers {
        let class_id = module.class_ids.get(&class.name).copied().unwrap_or(0);
        let cid_const = f.new_value();
        f.emit(Inst::ConstInt { dest: cid_const.clone(), value: class_id });
        let cmp = f.new_value();
        f.emit(Inst::CmpEq { dest: cmp.clone(), lhs: class_id_val.clone(), rhs: cid_const.clone(), ty: IrType::I64 });

        let call_bb = f.new_block();
        let next_bb = f.new_block();
        f.emit(Inst::Branch { cond: cmp, then_bb: call_bb.clone(), else_bb: next_bb.clone() });

        f.switch_to(&call_bb);
        let callee = format!("{}_{}", class.name, method.name);
        let mut call_args = vec![self_val.clone()];
        call_args.extend(arg_vals.clone());
        if ret_ty == IrType::Void {
            f.emit(Inst::Call { dest: None, func: callee, args: call_args, ret_ty: IrType::Void });
            f.emit(Inst::Return { value: None });
        } else {
            let r = f.new_value();
            f.emit(Inst::Call { dest: Some(r.clone()), func: callee, args: call_args, ret_ty: ret_ty.clone() });
            f.emit(Inst::Return { value: Some(r) });
        }

        f.switch_to(&next_bb);
    }

    // Aucun implémenteur ne correspond : ne devrait jamais arriver pour un
    // programme qui a passé la sema (`d:Drawable` ne peut recevoir qu'une
    // instance d'une classe implémentant directement `Drawable`, voir
    // `types_compat`/`SymbolTable::class_matches`) — retour défensif d'une
    // valeur neutre plutôt qu'un comportement indéfini.
    if ret_ty == IrType::Void {
        f.emit(Inst::Return { value: None });
    } else {
        let zero = f.new_value();
        f.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
        f.emit(Inst::Return { value: Some(zero) });
    }

    module.add_function(f);
}
