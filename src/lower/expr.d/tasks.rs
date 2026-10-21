/// Lancement d'une tâche `async` (fonction libre, méthode statique ou
/// d'instance, dispatcher) — voir docs/EBNF.md §14.5.

use crate::ir::inst::{Inst, Value};
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;

/// Range `args` dans un env heap et appelle
/// `__task_spawn(__async_wrap_<target>, env, forme du résultat)` : rend le
/// handle `Resolvable<T>`, valeur comptée possédée (+1).
pub fn spawn_task(builder: &mut LowerBuilder, target: &str, args: &[Value]) -> Value {
    let env_size = builder.new_value();
    builder.emit(Inst::ConstInt { dest: env_size.clone(), value: ((args.len() * 8).max(8)) as i64 });
    let env_ptr = builder.new_value();
    builder.emit(Inst::Call {
        dest:   Some(env_ptr.clone()),
        func:   "__alloc_obj".into(),
        args:   vec![env_size],
        ret_ty: IrType::I64,
    });
    crate::lower::stmt::rc::keep_task_args(builder, target, args);
    for (i, arg) in args.iter().enumerate() {
        builder.emit(Inst::SetField {
            obj:    env_ptr.clone(),
            field:  format!("__arg{}", i),
            src:    arg.clone(),
            offset: (i * 8) as i32,
        });
    }
    let func_addr = builder.new_value();
    builder.emit(Inst::FuncAddr { dest: func_addr.clone(), func: format!("__async_wrap_{}", target) });
    let result_kind = builder.new_value();
    let kind = builder.module.task_result_kinds.get(target).copied().unwrap_or(0);
    builder.emit(Inst::ConstInt { dest: result_kind.clone(), value: kind });
    let task = builder.new_value();
    builder.emit(Inst::Call {
        dest:   Some(task.clone()),
        func:   "__task_spawn".into(),
        args:   vec![func_addr, env_ptr, result_kind],
        ret_ty: IrType::Ptr,
    });
    task
}
