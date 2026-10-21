// Tâches `async` : handle compté, seconde résolution, abandon
// (docs/EBNF.md §14.5).

use crate::rc::*;
use crate::task::*;
use crate::*;

extern "C" fn answer(_env: i64) -> i64 {
    42
}

extern "C" fn ratio(_env: i64) -> i64 {
    __box_float(2.5f64.to_bits() as i64)
}

extern "C" fn text(_env: i64) -> i64 {
    unsafe { alloc_str("résultat") }
}

#[test]
fn second_resolve_gives_default_value() {
    let t = __task_spawn(answer as *const () as i64, 0, 0);
    assert_eq!(__rc_count(t), 1);
    assert_eq!(__task_resolve(t), 42);
    assert_eq!(__task_resolve(t), 0);
    __rc_release(t);
}

#[test]
fn second_resolve_of_float_gives_boxed_zero() {
    let t = __task_spawn(ratio as *const () as i64, 0, 2);
    let first = __task_resolve(t);
    assert_eq!(__unbox_float(first), 2.5);
    __rc_release(first);
    let second = __task_resolve(t);
    assert_eq!(__unbox_float(second), 0.0);
    __rc_release(second);
    __rc_release(t);
}

#[test]
fn shared_handle_is_resolved_once() {
    let t = __task_spawn(text as *const () as i64, 0, 1);
    __rc_retain(t);
    let s = __task_resolve(t);
    assert_eq!(unsafe { ptr_to_str(s) }, "résultat");
    __rc_release(s);
    __rc_release(t);
    assert_eq!(__task_resolve(t), 0);
    __rc_release(t);
}

#[test]
fn unresolved_task_is_released_with_its_handle() {
    for _ in 0..100 {
        let t = __task_spawn(text as *const () as i64, 0, 1);
        __rc_release(t);
    }
}
