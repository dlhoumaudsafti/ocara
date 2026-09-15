/// Lowering des littéraux et vérifications de type

use crate::parsing::ast::*;
use crate::ir::inst::{Inst, Value};
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;

/// Génère un test de type runtime : `val is Type` → bool
/// Appelle des fonctions runtime pour faire le check
pub fn lower_is_check(builder: &mut LowerBuilder, val: &Value, ty: &Type) -> Value {
    let runtime_func = match ty {
        Type::Null => "__is_null",
        Type::Int => "__is_int",
        Type::Float => "__is_float",
        Type::Bool => "__is_bool",
        Type::String => "__is_string",
        Type::Array(_) => "__is_array",
        Type::Map(_, _) => "__is_map",
        Type::Function { .. } => "__is_function",
        Type::Named(name) => return lower_class_or_interface_is_check(builder, val, name),
        Type::Qualified(parts) => {
            let name = parts.last().cloned().unwrap_or_default();
            return lower_class_or_interface_is_check(builder, val, &name);
        }
        _ => {
            // Pour les autres types (mixed, void, union), retourne false
            let dest = builder.new_value();
            builder.emit(Inst::ConstBool { dest: dest.clone(), value: false });
            return dest;
        }
    };

    // Appel de la fonction runtime de type check
    let dest = builder.new_value();
    builder.emit(Inst::Call {
        dest: Some(dest.clone()),
        func: runtime_func.into(),
        args: vec![val.clone()],
        ret_ty: IrType::I64,  // bool retourné comme i64
    });
    dest
}

/// `val is ClassName`/`val is InterfaceName` — RÉEL : jusqu'ici, les deux
/// compilaient en code strictement identique (`__is_object` seul), incapable
/// de distinguer une classe d'une autre ou une classe d'une interface (voir
/// docs/roadmap.d/langage-interfaces.md). Vérifie maintenant que `val` est un
/// objet ET que son identité de classe réelle (`class_id`, voir
/// `__alloc_class_obj` dans runtime/src/lib.rs) fait partie des candidats
/// précalculés pour `name` (`IrModule::is_check_candidates` — elle-même et
/// ses descendants pour une classe, ses implémenteurs directs pour une
/// interface). `name` inconnu (ni classe ni interface) : retombe sur
/// l'ancien comportement (`__is_object` seul), inchangé.
///
/// La lecture du `class_id` (`GetField` à offset -16) n'est sûre QUE si
/// `val` est réellement un objet — d'où le vrai branchement (pas un simple
/// `&&`, qui évaluerait les deux côtés sans court-circuit, voir
/// `Inst::And`) : un slot de pile porte le résultat à travers les deux
/// chemins, motif déjà utilisé ailleurs dans ce lowering pour toute valeur
/// devant survivre à une fusion de blocs.
fn lower_class_or_interface_is_check(builder: &mut LowerBuilder, val: &Value, name: &str) -> Value {
    let Some(candidate_ids) = builder.module.is_check_candidates.get(name).cloned() else {
        // Nom inconnu (ni classe ni interface) : comportement historique.
        let dest = builder.new_value();
        builder.emit(Inst::Call {
            dest: Some(dest.clone()),
            func: "__is_object".into(),
            args: vec![val.clone()],
            ret_ty: IrType::I64,
        });
        return dest;
    };

    let result_slot = builder.new_value();
    builder.emit(Inst::Alloca { dest: result_slot.clone(), ty: IrType::Bool });
    let false_val = builder.new_value();
    builder.emit(Inst::ConstBool { dest: false_val.clone(), value: false });
    builder.emit(Inst::Store { ptr: result_slot.clone(), src: false_val });

    let is_obj_raw = builder.new_value();
    builder.emit(Inst::Call {
        dest: Some(is_obj_raw.clone()),
        func: "__is_object".into(),
        args: vec![val.clone()],
        ret_ty: IrType::I64,
    });
    let zero = builder.new_value();
    builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
    let is_obj = builder.new_value();
    builder.emit(Inst::CmpNe { dest: is_obj.clone(), lhs: is_obj_raw, rhs: zero, ty: IrType::I64 });

    let check_bb = builder.new_block();
    let merge_bb = builder.new_block();
    builder.emit(Inst::Branch { cond: is_obj, then_bb: check_bb.clone(), else_bb: merge_bb.clone() });

    builder.switch_to(&check_bb);
    let class_id_val = builder.new_value();
    builder.emit(Inst::GetField {
        dest:   class_id_val.clone(),
        obj:    val.clone(),
        field:  "__class_id".into(),
        ty:     IrType::I64,
        offset: -16,
    });
    // `candidate_ids` ne peut pas être vide ici : `is_check_candidates`
    // contient toujours au moins l'id de la classe elle-même pour une
    // entrée "classe" — une entrée "interface" jamais implémentée n'est
    // simplement jamais insérée dans la table (voir program.rs), donc
    // `name` tomberait alors dans le cas "inconnu" ci-dessus.
    let mut match_val = {
        let id_const = builder.new_value();
        builder.emit(Inst::ConstInt { dest: id_const.clone(), value: candidate_ids[0] });
        let cmp = builder.new_value();
        builder.emit(Inst::CmpEq { dest: cmp.clone(), lhs: class_id_val.clone(), rhs: id_const, ty: IrType::I64 });
        cmp
    };
    for &id in &candidate_ids[1..] {
        let id_const = builder.new_value();
        builder.emit(Inst::ConstInt { dest: id_const.clone(), value: id });
        let cmp = builder.new_value();
        builder.emit(Inst::CmpEq { dest: cmp.clone(), lhs: class_id_val.clone(), rhs: id_const, ty: IrType::I64 });
        let combined = builder.new_value();
        builder.emit(Inst::Or { dest: combined.clone(), lhs: match_val, rhs: cmp });
        match_val = combined;
    }
    builder.emit(Inst::Store { ptr: result_slot.clone(), src: match_val });
    builder.emit(Inst::Jump { target: merge_bb.clone() });

    builder.switch_to(&merge_bb);
    let final_val = builder.new_value();
    builder.emit(Inst::Load { dest: final_val.clone(), ptr: result_slot.clone(), ty: IrType::Bool });
    final_val
}

pub fn lower_literal(builder: &mut LowerBuilder, lit: &Literal) -> Value {
    match lit {
        Literal::Int(n) => {
            let dest = builder.new_value();
            builder.emit(Inst::ConstInt { dest: dest.clone(), value: *n });
            dest
        }
        Literal::Float(f) => {
            let dest = builder.new_value();
            builder.emit(Inst::ConstFloat { dest: dest.clone(), value: *f });
            dest
        }
        Literal::Bool(b) => {
            let dest = builder.new_value();
            builder.emit(Inst::ConstBool { dest: dest.clone(), value: *b });
            dest
        }
        Literal::String(s) => {
            let idx = builder.module.intern_string(s);
            let dest = builder.new_value();
            builder.emit(Inst::ConstStr { dest: dest.clone(), idx });
            dest
        }
        Literal::Null => {
            let dest = builder.new_value();
            builder.emit(Inst::ConstInt { dest: dest.clone(), value: 0 });
            dest
        }
    }
}
