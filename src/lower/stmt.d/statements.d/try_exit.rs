/// Sorties d'un corps `try` ou d'un gestionnaire `on` (fonctions séparées,
/// voir `lower_try`) : `return`, `break` et `continue` y rendent un code de
/// sortie (`__try_body_N`/`__try_handler_N` → `__ocara_try_exec`), que la
/// fonction englobante exécute après l'appel. La valeur d'un `return` passe
/// par `__ocara_try_set_return`/`__ocara_try_take_return`.

use crate::ir::inst::{BlockId, Inst, Value};
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;

pub const TRY_NORMAL: i64 = 0;
pub const TRY_RETURN: i64 = 1;
pub const TRY_BREAK: i64 = 2;
pub const TRY_CONTINUE: i64 = 3;

/// Vrai dans le corps d'un `try` ou un gestionnaire `on`.
pub fn in_try_fn(builder: &LowerBuilder) -> bool {
    builder.func.name.starts_with("__try_body_") || builder.func.name.starts_with("__try_handler_")
}

fn const_int(builder: &mut LowerBuilder, value: i64) -> Value {
    let v = builder.new_value();
    builder.emit(Inst::ConstInt { dest: v.clone(), value });
    v
}

/// Quitte le corps/gestionnaire avec `code` ; `value` : valeur du `return`.
/// Les locales doivent déjà être rendues.
pub fn exit_try_fn(builder: &mut LowerBuilder, code: i64, value: Option<Value>) {
    if let Some(v) = value {
        builder.emit(Inst::Call { dest: None, func: "__ocara_try_set_return".into(), args: vec![v], ret_ty: IrType::Void });
    }
    let c = const_int(builder, code);
    builder.emit(Inst::Return { value: Some(c) });
}

/// Après `__ocara_try_exec` : exécute la sortie demandée par le corps ou le
/// gestionnaire, puis continue dans un nouveau bloc (fin normale).
pub fn dispatch_try_exit(builder: &mut LowerBuilder, code: Value) {
    let continue_bb = builder.new_block();
    let exit_bb = builder.new_block();
    let normal = const_int(builder, TRY_NORMAL);
    let is_normal = builder.new_value();
    builder.emit(Inst::CmpEq { dest: is_normal.clone(), lhs: code.clone(), rhs: normal, ty: IrType::I64 });
    builder.emit(Inst::Branch { cond: is_normal, then_bb: continue_bb.clone(), else_bb: exit_bb.clone() });

    // Les destructions anticipées émises sur un chemin de sortie ne
    // concernent que lui (voir `lower_if`).
    let owned_snapshot = builder.owned_locals.clone();
    builder.switch_to(&exit_bb);
    let loop_exit = !builder.loop_stack.is_empty() || in_try_fn(builder);
    if loop_exit {
        let return_bb = builder.new_block();
        let other_bb = builder.new_block();
        let break_bb = builder.new_block();
        let continue_loop_bb = builder.new_block();
        branch_on(builder, &code, TRY_RETURN, &return_bb, &other_bb);
        builder.switch_to(&other_bb);
        branch_on(builder, &code, TRY_BREAK, &break_bb, &continue_loop_bb);
        builder.switch_to(&break_bb);
        super::loops::lower_break(builder);
        builder.owned_locals = owned_snapshot.clone();
        builder.switch_to(&continue_loop_bb);
        super::loops::lower_continue(builder);
        builder.owned_locals = owned_snapshot.clone();
        builder.switch_to(&return_bb);
    }
    lower_try_return(builder, &continue_bb);
    builder.owned_locals = owned_snapshot;
    builder.switch_to(&continue_bb);
}

fn branch_on(builder: &mut LowerBuilder, code: &Value, expected: i64, then_bb: &BlockId, else_bb: &BlockId) {
    let k = const_int(builder, expected);
    let hit = builder.new_value();
    builder.emit(Inst::CmpEq { dest: hit.clone(), lhs: code.clone(), rhs: k, ty: IrType::I64 });
    builder.emit(Inst::Branch { cond: hit, then_bb: then_bb.clone(), else_bb: else_bb.clone() });
}

/// `return` demandé par le corps ou le gestionnaire : propagé tel quel par
/// un `try` englobant, sinon exécuté avec la valeur transmise (`main`
/// runtime : `ERROR` puis sortie anticipée).
fn lower_try_return(builder: &mut LowerBuilder, continue_bb: &BlockId) {
    if in_try_fn(builder) {
        crate::lower::stmt::rc::release_all(builder);
        exit_try_fn(builder, TRY_RETURN, None);
        return;
    }
    let value = builder.new_value();
    builder.emit(Inst::Call { dest: Some(value.clone()), func: "__ocara_try_take_return".into(), args: vec![], ret_ty: IrType::I64 });
    if builder.func.name == "main" {
        if let (Some((error_slot, _, _)), Some(exit_bb)) = (builder.locals.get("ERROR").cloned(), builder.runtime_exit_bb.clone()) {
            builder.emit(Inst::Store { ptr: error_slot, src: value });
            crate::lower::stmt::rc::release_before_runtime_exit(builder);
            builder.emit(Inst::Jump { target: exit_bb });
            return;
        }
        if builder.runtime_exit_bb.is_some() {
            builder.emit(Inst::Jump { target: continue_bb.clone() });
            return;
        }
    }
    crate::lower::stmt::rc::release_all(builder);
    let ret = (builder.func.ret_ty != IrType::Void).then_some(value);
    builder.emit(Inst::Return { value: ret });
}
