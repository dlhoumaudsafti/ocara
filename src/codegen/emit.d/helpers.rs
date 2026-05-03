/// Fonctions utilitaires pour le codegen

use cranelift_codegen::ir::types as clt;
use crate::ir::func::IrFunction;
use crate::ir::inst::{Inst, Value};
use crate::ir::types::IrType;

/// Convertit un type IR Ocara en type Cranelift
pub fn ir_type_to_cl(ty: &IrType) -> cranelift_codegen::ir::Type {
    match ty {
        IrType::I64  => clt::I64,
        IrType::F64  => clt::F64,
        IrType::Bool => clt::I64,
        IrType::Ptr  => clt::I64,
        IrType::Void => clt::I64,
    }
}

/// Calcule le plus grand ID de valeur HIR utilisé dans une fonction
pub fn max_value_id(func: &IrFunction) -> u32 {
    let mut max = 0u32;
    for bb in &func.blocks {
        for inst in &bb.insts {
            visit_values(inst, |v: &Value| {
                if v.0 > max { max = v.0; }
            });
        }
    }
    max
}

/// Applique une fonction à toutes les valeurs utilisées dans une instruction
pub fn visit_values<F: FnMut(&Value)>(inst: &Inst, mut f: F) {
    macro_rules! v { ($val:expr) => { f($val) } }
    match inst {
        Inst::ConstInt   { dest, .. }       => { v!(dest); }
        Inst::ConstFloat { dest, .. }       => { v!(dest); }
        Inst::ConstBool  { dest, .. }       => { v!(dest); }
        Inst::ConstStr   { dest, .. }       => { v!(dest); }
        Inst::Add        { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::Sub        { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::Mul        { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::Div        { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::Mod        { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::Neg        { dest, src, .. }  => { v!(dest); v!(src); }
        Inst::CmpEq      { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::CmpNe      { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::CmpLt      { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::CmpLe      { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::CmpGt      { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::CmpGe      { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::And        { dest, lhs, rhs } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::Or         { dest, lhs, rhs } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::Not        { dest, src }      => { v!(dest); v!(src); }
        Inst::Alloca     { dest, .. }       => { v!(dest); }
        Inst::Store      { ptr, src }       => { v!(ptr); v!(src); }
        Inst::Load       { dest, ptr, .. }  => { v!(dest); v!(ptr); }
        Inst::Call       { dest, args, .. } => {
            if let Some(d) = dest { v!(d); }
            args.iter().for_each(|a| v!(a));
        }
        Inst::CallIndirect { dest, callee, args, .. } => {
            if let Some(d) = dest { v!(d); }
            v!(callee);
            args.iter().for_each(|a| v!(a));
        }
        Inst::Jump { .. }                   => {}
        Inst::Branch { cond, .. }           => { v!(cond); }
        Inst::Return { value }              => { if let Some(val) = value { v!(val); } }
        Inst::Phi    { dest, sources, .. }  => {
            v!(dest);
            sources.iter().for_each(|(val, _)| v!(val));
        }
        Inst::Alloc    { dest, .. }         => { v!(dest); }
        Inst::SetField { obj, src, .. }     => { v!(obj); v!(src); }
        Inst::GetField { dest, obj, .. }    => { v!(dest); v!(obj); }
        Inst::FuncAddr { dest, .. }         => { v!(dest); }
        Inst::Nop                           => {}
    }
}
