/// Correspondance valeur HIR → valeur Cranelift d'une fonction.
///
/// Une valeur définie une seule fois et utilisée seulement plus loin dans le
/// même bloc (le cas de presque tous les temporaires) est une simple valeur
/// Cranelift ; les autres (paramètres, `Phi`, valeurs vivantes d'un bloc à
/// l'autre) passent par une `Variable` du constructeur SSA. Tout déclarer en
/// `Variable` coûtait, dans `cranelift-frontend`, une table par variable
/// indexée par bloc : mémoire et temps quadratiques sur une longue chaîne
/// `elseif` (10 000 branches : 1,6 Go).
use std::cell::RefCell;
use cranelift_codegen::ir::{types as clt, Value as ClValue};
use cranelift_frontend::{FunctionBuilder, Variable};
use crate::ir::func::IrFunction;
use crate::ir::inst::{Inst, Value};
use super::helpers::{max_value_id, visit_values};

pub struct ValueSlots {
    block_local: Vec<bool>,
    locals: RefCell<Vec<Option<ClValue>>>,
}

#[derive(Clone, Copy)]
struct Occurrences {
    defs: u32,
    def_at: (usize, usize),
    local: bool,
}

impl ValueSlots {
    pub fn new(builder: &mut FunctionBuilder, ir_func: &IrFunction) -> Self {
        let count = max_value_id(ir_func) as usize + 1;
        let mut occ = vec![Occurrences { defs: 0, def_at: (0, 0), local: true }; count];
        for param in &ir_func.params {
            if let Some(o) = occ.get_mut(param.slot.0 as usize) {
                o.local = false;
            }
        }
        for (b, bb) in ir_func.blocks.iter().enumerate() {
            for (p, inst) in bb.insts.iter().enumerate() {
                let dest = inst_dest(inst).map(|d| d.0 as usize);
                if let Inst::Phi { dest, sources, .. } = inst {
                    occ[dest.0 as usize].local = false;
                    for (source, _) in sources {
                        occ[source.0 as usize].local = false;
                    }
                }
                if let Some(d) = dest {
                    occ[d].defs += 1;
                    occ[d].def_at = (b, p);
                }
                let mut dest_seen = false;
                visit_values(inst, |v| {
                    let i = v.0 as usize;
                    if Some(i) == dest && !dest_seen {
                        dest_seen = true;
                        return;
                    }
                    let o = &mut occ[i];
                    if o.defs == 0 || o.def_at.0 != b || o.def_at.1 >= p {
                        o.local = false;
                    }
                });
            }
        }
        let block_local: Vec<bool> = occ.iter().map(|o| o.local && o.defs == 1).collect();
        for (i, local) in block_local.iter().enumerate() {
            if !local {
                builder.declare_var(Variable::from_u32(i as u32), clt::I64);
            }
        }
        ValueSlots { locals: RefCell::new(vec![None; count]), block_local }
    }

    pub fn len(&self) -> usize {
        self.block_local.len()
    }

    pub fn def(&self, builder: &mut FunctionBuilder, v: u32, val: ClValue) {
        if self.block_local[v as usize] {
            self.locals.borrow_mut()[v as usize] = Some(val);
        } else {
            builder.def_var(Variable::from_u32(v), val);
        }
    }

    pub fn get(&self, builder: &mut FunctionBuilder, v: u32) -> ClValue {
        if self.block_local[v as usize] {
            self.locals.borrow()[v as usize].expect("valeur locale à un bloc lue avant sa définition")
        } else {
            builder.use_var(Variable::from_u32(v))
        }
    }
}

fn inst_dest(inst: &Inst) -> Option<&Value> {
    match inst {
        Inst::ConstInt { dest, .. } | Inst::ConstFloat { dest, .. } | Inst::ConstBool { dest, .. }
        | Inst::ConstStr { dest, .. }
        | Inst::Add { dest, .. } | Inst::Sub { dest, .. } | Inst::Mul { dest, .. } | Inst::Div { dest, .. }
        | Inst::Mod { dest, .. } | Inst::Neg { dest, .. }
        | Inst::CmpEq { dest, .. } | Inst::CmpNe { dest, .. } | Inst::CmpLt { dest, .. }
        | Inst::CmpLe { dest, .. } | Inst::CmpGt { dest, .. } | Inst::CmpGe { dest, .. }
        | Inst::And { dest, .. } | Inst::Or { dest, .. } | Inst::Not { dest, .. }
        | Inst::Alloca { dest, .. } | Inst::AllocaWords { dest, .. } | Inst::Load { dest, .. }
        | Inst::Phi { dest, .. } | Inst::Alloc { dest, .. } | Inst::GetField { dest, .. }
        | Inst::FuncAddr { dest, .. } => Some(dest),
        Inst::Call { dest, .. } | Inst::CallIndirect { dest, .. } => dest.as_ref(),
        Inst::Store { .. } | Inst::Jump { .. } | Inst::Branch { .. } | Inst::Return { .. }
        | Inst::SetField { .. } | Inst::Nop => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::Function;
    use cranelift_frontend::FunctionBuilderContext;
    use crate::ir::func::IrParam;
    use crate::ir::types::IrType;

    fn block_local(ir: &IrFunction) -> Vec<bool> {
        let mut func = Function::new();
        let mut ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut func, &mut ctx);
        ValueSlots::new(&mut builder, ir).block_local
    }

    #[test]
    fn temporaries_are_local_and_cross_block_values_are_variables() {
        let mut f = IrFunction::new("f".into(), vec![], IrType::I64);
        let param = f.new_value();
        f.params = vec![IrParam { name: "x".into(), ty: IrType::I64, slot: param.clone() }];
        let one = f.new_value();
        let sum = f.new_value();
        let kept = f.new_value();
        f.emit(Inst::ConstInt { dest: one.clone(), value: 1 });
        f.emit(Inst::Add { dest: sum.clone(), lhs: param.clone(), rhs: one.clone(), ty: IrType::I64 });
        f.emit(Inst::Add { dest: kept.clone(), lhs: sum.clone(), rhs: one.clone(), ty: IrType::I64 });
        let next = f.new_block();
        f.emit(Inst::Jump { target: next.clone() });
        f.switch_to(&next);
        f.emit(Inst::Return { value: Some(kept.clone()) });

        let local = block_local(&f);
        assert!(!local[param.0 as usize], "a parameter is always a variable");
        assert!(local[one.0 as usize]);
        assert!(local[sum.0 as usize]);
        assert!(!local[kept.0 as usize], "used in another block");
    }

    #[test]
    fn value_defined_twice_is_a_variable() {
        let mut f = IrFunction::new("f".into(), vec![], IrType::I64);
        let v = f.new_value();
        f.emit(Inst::ConstInt { dest: v.clone(), value: 1 });
        f.emit(Inst::ConstInt { dest: v.clone(), value: 2 });
        f.emit(Inst::Return { value: Some(v.clone()) });
        assert!(!block_local(&f)[v.0 as usize]);
    }
}
