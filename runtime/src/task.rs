//! Tâches `async` : une valeur `Resolvable<T>` est un bloc compté
//! (`TAG_TASK`) dont l'état est partagé avec le thread de la tâche — voir
//! docs/roadmap.d/async-resolvable-handles.md.
//!
//! - `resolve` attend le résultat et le transfère à l'appelant ; une seconde
//!   résolution rend la valeur par défaut de `T`, sans rien libérer.
//! - La dernière référence rendue avant `resolve` abandonne la tâche : elle
//!   s'exécute quand même, et relâche elle-même son résultat.
//!
//! Exécution (voir docs/roadmap.d/perf-runtime-complexite.md) : un pool de
//! threads réutilisés — un thread inactif reprend la tâche suivante, et un
//! nouveau thread est créé quand aucun n'est libre (une tâche qui en attend
//! une autre ne peut donc pas bloquer le pool). Un `resolve` sur une tâche
//! qu'aucun thread n'a encore prise l'exécute lui-même, sur place.

use std::collections::VecDeque;
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
    /// Fonction et environnement de la tâche, tant qu'elle n'a pas démarré.
    job: Mutex<Option<(i64, i64)>>,
    result: i64,
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

/// Exécute la tâche si elle n'a pas encore démarré ; rend son issue, ou
/// `None` si un autre thread l'a déjà prise.
fn run_job(shared: &Shared) -> Option<Outcome> {
    let (func, env) = shared.job.lock().unwrap_or_else(|e| e.into_inner()).take()?;
    // Une exception non rattrapée dans la tâche est capturée ici (au lieu
    // d'arrêter le processus) et relevée au `resolve`.
    Some(crate::run_closure_catching(func, env))
}

/// Tâche exécutée par un thread du pool : son issue est rangée pour le
/// `resolve`, ou relâchée si la tâche a été abandonnée.
fn run_queued(shared: &Shared) {
    let _entered = rc::enter();
    let Some(outcome) = run_job(shared) else { return };
    let mut slot = lock(shared);
    if matches!(*slot, Slot::Abandoned) {
        drop(slot);
        release_outcome(outcome, shared.result);
    } else {
        *slot = Slot::Done(outcome);
        shared.done.notify_all();
    }
    PENDING.fetch_sub(1, Ordering::SeqCst);
}

struct PoolState {
    queue: VecDeque<Arc<Shared>>,
    /// Threads en attente d'une tâche.
    idle: usize,
    /// Réveils promis à des threads inactifs, chacun pour une tâche de la file.
    wakeups: usize,
}

static POOL: Mutex<PoolState> = Mutex::new(PoolState { queue: VecDeque::new(), idle: 0, wakeups: 0 });
static POOL_READY: Condvar = Condvar::new();
/// Un thread du pool inactif depuis ce délai se termine.
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);

fn pool() -> std::sync::MutexGuard<'static, PoolState> {
    POOL.lock().unwrap_or_else(|e| e.into_inner())
}

/// Confie la tâche à un thread inactif, ou à un nouveau thread.
fn submit(shared: Arc<Shared>) {
    rc::go_multi_threaded();
    let mut state = pool();
    state.queue.push_back(shared);
    if state.idle > 0 {
        state.idle -= 1;
        state.wakeups += 1;
        POOL_READY.notify_one();
    } else {
        drop(state);
        std::thread::spawn(worker);
    }
}

/// Thread du pool : chaque tâche de la file lui revient soit à sa création,
/// soit par un réveil promis ; il attend ensuite la suivante.
fn worker() {
    let _stack = crate::stack_guard::ThreadStack::enter();
    let mut next = pool().queue.pop_front();
    while let Some(shared) = next.take() {
        run_queued(&shared);
        drop(shared);
        let mut state = pool();
        state.idle += 1;
        loop {
            let (guard, timeout) = POOL_READY.wait_timeout(state, IDLE_TIMEOUT).unwrap_or_else(|e| e.into_inner());
            state = guard;
            if state.wakeups > 0 {
                state.wakeups -= 1;
                next = state.queue.pop_front();
                break;
            }
            if timeout.timed_out() {
                state.idle -= 1;
                break;
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __task_spawn(func: i64, env: i64, result: i64) -> i64 {
    let shared = Arc::new(Shared {
        slot: Mutex::new(Slot::Running),
        done: Condvar::new(),
        job: Mutex::new(Some((func, env))),
        result,
    });
    PENDING.fetch_add(1, Ordering::SeqCst);
    submit(Arc::clone(&shared));
    unsafe {
        let ptr = rc::alloc_block(size_of::<OcaraTask>(), 0, crate::typecheck::TAG_TASK, false);
        std::ptr::write(ptr as *mut OcaraTask, OcaraTask { shared, result });
        ptr
    }
}

// `resolve expr` : attend la fin de la tâche et transfère son résultat — ou
// l'exécute sur place si aucun thread ne l'a encore prise. Une exception de
// la tâche est relevée ici, rattrapable par le `try` qui entoure `resolve`
// (la valeur levée est transférée).
#[unsafe(no_mangle)]
pub extern "C" fn __task_resolve(task_ptr: i64) -> i64 {
    if task_ptr == 0 {
        return 0;
    }
    let task = unsafe { &*(task_ptr as *const OcaraTask) };
    let outcome = match run_job(&task.shared) {
        Some(outcome) => {
            *lock(&task.shared) = Slot::Taken;
            PENDING.fetch_sub(1, Ordering::SeqCst);
            Slot::Done(outcome)
        }
        None => {
            let _parked = rc::park();
            let mut slot = lock(&task.shared);
            while matches!(*slot, Slot::Running) {
                slot = task.shared.done.wait(slot).unwrap_or_else(|e| e.into_inner());
            }
            std::mem::replace(&mut *slot, Slot::Taken)
        }
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
