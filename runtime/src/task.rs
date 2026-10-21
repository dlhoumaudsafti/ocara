//! Tâches `async` : une valeur `Resolvable<T>` est un bloc compté
//! (`TAG_TASK`) dont l'état est partagé avec le thread de la tâche — voir
//! docs/roadmap.d/async-resolvable-handles.md.
//!
//! - `resolve` attend le résultat et le transfère à l'appelant ; une seconde
//!   résolution rend la valeur par défaut de `T`, sans rien libérer.
//! - La dernière référence rendue avant `resolve` abandonne la tâche : le
//!   thread (détaché dès le lancement) relâche lui-même son résultat.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::rc;

/// Forme du résultat rendu par le wrapper de la tâche (`__async_wrap_*`),
/// choisie par le compilateur au site d'appel.
const RESULT_RAW: i64 = 0;
const RESULT_FLOAT: i64 = 2;
const RESULT_BOOL: i64 = 3;

type Outcome = Result<i64, (i64, i64)>;

enum Slot {
    Running,
    Done(Outcome),
    Taken,
    Abandoned,
}

struct Shared {
    slot: Mutex<Slot>,
    done: Condvar,
}

struct OcaraTask {
    shared: Arc<Shared>,
    result: i64,
}

/// Tâches dont le thread n'a pas encore rendu son résultat.
static PENDING: AtomicUsize = AtomicUsize::new(0);

fn lock(shared: &Shared) -> std::sync::MutexGuard<'_, Slot> {
    shared.slot.lock().unwrap_or_else(|e| e.into_inner())
}

/// Relâche le résultat d'une tâche que plus personne ne résoudra.
fn release_outcome(outcome: Outcome, result: i64) {
    match outcome {
        Ok(value) if result != RESULT_RAW => rc::__rc_release(value),
        Ok(_) => {}
        Err((value, _)) => rc::__rc_release(value),
    }
}

/// Valeur rendue par une résolution sans résultat : la valeur par défaut de
/// `T`, dans la forme que le site de `resolve` déballe.
fn default_result(result: i64) -> i64 {
    match result {
        RESULT_FLOAT => crate::__box_float(0.0f64.to_bits() as i64),
        RESULT_BOOL => crate::__box_bool(0),
        _ => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __task_spawn(func: i64, env: i64, result: i64) -> i64 {
    let shared = Arc::new(Shared { slot: Mutex::new(Slot::Running), done: Condvar::new() });
    let worker = Arc::clone(&shared);
    let guard = rc::ThreadGuard::new();
    PENDING.fetch_add(1, Ordering::SeqCst);
    std::thread::spawn(move || {
        let _guard = guard;
        let _stack = crate::stack_guard::ThreadStack::enter();
        // Une exception non rattrapée dans la tâche est capturée ici (au
        // lieu d'arrêter le processus) et relevée au `resolve`.
        let outcome = crate::run_closure_catching(func, env);
        let mut slot = lock(&worker);
        if matches!(*slot, Slot::Abandoned) {
            drop(slot);
            release_outcome(outcome, result);
        } else {
            *slot = Slot::Done(outcome);
            worker.done.notify_all();
        }
        PENDING.fetch_sub(1, Ordering::SeqCst);
    });
    unsafe {
        let ptr = rc::alloc_block(size_of::<OcaraTask>(), 0, crate::typecheck::TAG_TASK, false);
        std::ptr::write(ptr as *mut OcaraTask, OcaraTask { shared, result });
        ptr
    }
}

// `resolve expr` : attend la fin de la tâche et transfère son résultat. Une
// exception de la tâche est relevée ici, rattrapable par le `try` qui
// entoure `resolve` (la valeur levée est transférée).
#[unsafe(no_mangle)]
pub extern "C" fn __task_resolve(task_ptr: i64) -> i64 {
    if task_ptr == 0 {
        return 0;
    }
    let task = unsafe { &*(task_ptr as *const OcaraTask) };
    let outcome = {
        let _parked = rc::park();
        let mut slot = lock(&task.shared);
        while matches!(*slot, Slot::Running) {
            slot = task.shared.done.wait(slot).unwrap_or_else(|e| e.into_inner());
        }
        std::mem::replace(&mut *slot, Slot::Taken)
    };
    match outcome {
        Slot::Done(Ok(value)) => value,
        Slot::Done(Err((ev, et))) => {
            crate::__ocara_fail(ev, et);
            0
        }
        _ => default_result(task.result),
    }
}

/// Dernière référence rendue : un résultat jamais résolu est relâché ici,
/// ou par le thread de la tâche s'il n'a pas encore fini.
pub(crate) unsafe fn free_task(val: i64) {
    unsafe {
        let task = std::ptr::read(val as *const OcaraTask);
        let outcome = std::mem::replace(&mut *lock(&task.shared), Slot::Abandoned);
        if let Slot::Done(outcome) = outcome {
            release_outcome(outcome, task.result);
        }
        rc::free_block(val, size_of::<OcaraTask>());
    }
}

/// Bilan `--leak-check` : laisse aux tâches abandonnées encore en cours le
/// temps de relâcher leur résultat.
pub(crate) fn settle() {
    let deadline = Instant::now() + Duration::from_secs(2);
    while PENDING.load(Ordering::SeqCst) > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
}
