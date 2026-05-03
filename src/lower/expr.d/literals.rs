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
        Type::Named(_) | Type::Qualified(_) => "__is_object",
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
