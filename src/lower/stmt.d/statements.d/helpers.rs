/// Helpers pour le lowering des statements

use crate::ir::inst::{Inst, Value};
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;

/// Si la variable cible est de type `mixed` (Ptr) et la valeur est F64 ou Bool,
/// on la boxe pour éviter que les bits soient interprétés comme un pointeur.
pub fn box_for_any(builder: &mut LowerBuilder, target_ty: &IrType, val_ty: IrType, val: Value) -> Value {
    if *target_ty != IrType::Ptr { return val; }
    match val_ty {
        IrType::F64 => {
            let d = builder.new_value();
            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__box_float".into(), args: vec![val], ret_ty: IrType::Ptr });
            d
        }
        IrType::Bool => {
            let d = builder.new_value();
            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__box_bool".into(), args: vec![val], ret_ty: IrType::Ptr });
            d
        }
        _ => val,
    }
}
