//! Assertions `UnitTest` d'un `RuntimeTest.oc` et assertions résolues à la
//! compilation.
//!
//! ocaraunit exécute le `main` des blocs runtime du point d'entrée puis,
//! dans le bloc `exit`, transmet `ERROR` (`__recordRuntimeResult`) avant
//! d'appeler les tests ; `assertSuccessResult`/`assertErrorResult`/
//! `assertErrorResultCode` comparent ce résultat. `assertBlocksMainThread`/
//! `assertIsBlocking` sont tranchées par le compilateur, qui les remplace
//! par `__staticAssert` (voir `src/sema/blocking_assert.rs`).

use std::sync::atomic::{AtomicI64, Ordering};

use crate::{ptr_to_str, ut_pass};

/// Aucun flot runtime exécuté : test ordinaire, pas un `RuntimeTest.oc`.
const NO_RESULT: i64 = i64::MIN;

static RESULT: AtomicI64 = AtomicI64::new(NO_RESULT);

const NO_RUNTIME_FLOW: i64 = 130;

fn fail(message: &str, code: i64) -> ! {
    unsafe { crate::exception::throw_unittest_exception(message, code) }
}

/// `ERROR` à la fin du flot runtime (`result` du `main`).
fn runtime_result(assertion: &str) -> i64 {
    let result = RESULT.load(Ordering::Relaxed);
    if result == NO_RESULT {
        fail(&format!("{}: no runtime flow was executed — only available in a RuntimeTest.oc", assertion), NO_RUNTIME_FLOW);
    }
    result
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest___recordRuntimeResult(code: i64) {
    RESULT.store(code, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertSuccessResult() {
    match runtime_result("assertSuccessResult") {
        0 => ut_pass("assertSuccessResult"),
        code => fail(&format!("assertSuccessResult: the runtime flow ended with 'result {}'", code), 127),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertErrorResult() {
    match runtime_result("assertErrorResult") {
        0 => fail("assertErrorResult: the runtime flow ended successfully", 128),
        code => ut_pass(&format!("assertErrorResult: result {}", code)),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest_assertErrorResultCode(expected: i64) {
    let code = runtime_result("assertErrorResultCode");
    if code == expected {
        ut_pass(&format!("assertErrorResultCode: result {}", code));
    } else {
        fail(&format!("assertErrorResultCode: expected result {} but the runtime flow ended with {}", expected, code), 129);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn UnitTest___staticAssert(ok: i64, message: i64, code: i64) {
    let message = unsafe { ptr_to_str(message) };
    if ok != 0 {
        ut_pass(message);
    } else {
        fail(message, code);
    }
}
