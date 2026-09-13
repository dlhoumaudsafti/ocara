/// Helpers pour le lowering des statements

use crate::ir::inst::{Inst, Value};
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;

/// Si la variable cible est de type `mixed` (Ptr) et la valeur est F64 ou Bool,
/// on la boxe pour éviter que les bits soient interprétés comme un pointeur.
///
/// Sens inverse : la cible est CONCRÈTE (int/float/bool) mais `val_ty` est
/// `Ptr` — ne peut arriver que si `val` provient en réalité d'un `mixed`
/// (jamais d'un vrai `string`/`array`/`map`/objet, dont l'affectation à une
/// cible concrète est refusée par la sema) : un `float`/`bool` boxé, ou un
/// `int` déjà brut. Sans déballage, le bit pattern du pointeur boxé était
/// stocké tel quel dans la cible concrète — résultat faux, confirmé par
/// reproduction (`var m:mixed = 3.5; var f:float = m` ; voir
/// docs/roadmap.d/langage-mixed-arithmetic.md, découvert en corrigeant
/// l'arithmétique `mixed` mais pas spécifique à un opérateur).
pub fn box_for_any(builder: &mut LowerBuilder, target_ty: &IrType, val_ty: IrType, val: Value) -> Value {
    if *target_ty != IrType::Ptr {
        if val_ty != IrType::Ptr {
            return val;
        }
        let (func, ret_ty) = match target_ty {
            IrType::F64 => ("__mixed_to_float", IrType::F64),
            _           => ("__mixed_to_int",   IrType::I64),
        };
        let d = builder.new_value();
        builder.emit(Inst::Call { dest: Some(d.clone()), func: func.into(), args: vec![val], ret_ty });
        return d;
    }
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
