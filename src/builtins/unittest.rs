// ─────────────────────────────────────────────────────────────────────────────
// ocara.UnitTest — classe builtin statique pour les tests unitaires
//
// Méthodes statiques :
//   UnitTest::assertEquals(expected, actual)       → void
//   UnitTest::assertNotEquals(expected, actual)    → void
//   UnitTest::assertTrue(value)                    → void
//   UnitTest::assertFalse(value)                   → void
//   UnitTest::assertNull(value)                    → void
//   UnitTest::assertNotNull(value)                 → void
//   UnitTest::assertGreater(a, b)                  → void
//   UnitTest::assertLess(a, b)                     → void
//   UnitTest::assertGreaterOrEquals(a, b)          → void
//   UnitTest::assertLessOrEquals(a, b)             → void
//   UnitTest::assertContains(haystack, needle)     → void
//   UnitTest::assertEmpty(value)                   → void
//   UnitTest::assertNotEmpty(value)                → void
//   UnitTest::fail(message)                        → void
//   UnitTest::pass(message)                        → void
//   UnitTest::assertRaises(callable)               → Exception
//   UnitTest::assertExceptionMessageEquals(message, expected)     → void
//   UnitTest::assertExceptionMessageNotEquals(message, expected)  → void
//   UnitTest::assertExceptionCodeEquals(code, expected)           → void
//   UnitTest::assertExceptionCodeNotEquals(code, expected)        → void
//   UnitTest::assertExceptionSourceEquals(source, expected)       → void
//   UnitTest::assertExceptionSourceNotEquals(source, expected)    → void
//   UnitTest::assertSuccessResult()                → void   (RuntimeTest.oc)
//   UnitTest::assertErrorResult()                  → void   (RuntimeTest.oc)
//   UnitTest::assertErrorResultCode(code)          → void   (RuntimeTest.oc)
//   UnitTest::assertBlocksMainThread(expr)         → void   (tranchée à la compilation)
//   UnitTest::assertIsBlocking(expr)               → void   (tranchée à la compilation)
//
// Internes (appels générés par ocaraunit et par la sema) :
//   UnitTest::__recordRuntimeResult(code)          → void
//   UnitTest::__staticAssert(ok, message, code)    → void
//   UnitTest::__reportFailure(label, message)      → void
//
// Convention runtime : UnitTest_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::parsing::ast::Type;
use crate::sema::symbols::{ClassInfo, FuncSig};

fn m(params: Vec<(&str, Type)>, ret_ty: Type) -> FuncSig {
    let len = params.len();
    FuncSig {
        params:    params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
        ret_ty,
        is_static: true,
        is_async:  false,
        has_variadic: false,
        fixed_params_count: len,
        required_params_count: len,
        message_emit_in_loop: false,
    }
}

pub fn class() -> ClassInfo {
    let mut methods: HashMap<String, FuncSig> = HashMap::new();

    // UnitTest::assertEquals(expected, actual) → void
    methods.insert("assertEquals".into(), m(
        vec![("expected", Type::Mixed), ("actual", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertNotEquals(expected, actual) → void
    methods.insert("assertNotEquals".into(), m(
        vec![("expected", Type::Mixed), ("actual", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertTrue(value) → void
    methods.insert("assertTrue".into(), m(
        vec![("value", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertFalse(value) → void
    methods.insert("assertFalse".into(), m(
        vec![("value", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertNull(value) → void
    methods.insert("assertNull".into(), m(
        vec![("value", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertNotNull(value) → void
    methods.insert("assertNotNull".into(), m(
        vec![("value", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertGreater(a, b) → void
    methods.insert("assertGreater".into(), m(
        vec![("a", Type::Mixed), ("b", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertLess(a, b) → void
    methods.insert("assertLess".into(), m(
        vec![("a", Type::Mixed), ("b", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertGreaterOrEquals(a, b) → void
    methods.insert("assertGreaterOrEquals".into(), m(
        vec![("a", Type::Mixed), ("b", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertLessOrEquals(a, b) → void
    methods.insert("assertLessOrEquals".into(), m(
        vec![("a", Type::Mixed), ("b", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertContains(haystack, needle) → void
    methods.insert("assertContains".into(), m(
        vec![("haystack", Type::Mixed), ("needle", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertEmpty(value) → void
    methods.insert("assertEmpty".into(), m(
        vec![("value", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertNotEmpty(value) → void
    methods.insert("assertNotEmpty".into(), m(
        vec![("value", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::fail(message) → void
    methods.insert("fail".into(), m(
        vec![("message", Type::String)],
        Type::Void,
    ));

    // UnitTest::pass(message) → void
    methods.insert("pass".into(), m(
        vec![("message", Type::String)],
        Type::Void,
    ));

    // UnitTest::assertFunction(value) → void
    methods.insert("assertFunction".into(), m(
        vec![("value", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertClass(value) → void
    methods.insert("assertClass".into(), m(
        vec![("value", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertEnum(value) → void
    methods.insert("assertEnum".into(), m(
        vec![("value", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertMap(value) → void
    methods.insert("assertMap".into(), m(
        vec![("value", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertArray(value) → void
    methods.insert("assertArray".into(), m(
        vec![("value", Type::Mixed)],
        Type::Void,
    ));

    // UnitTest::assertRaises(callable) → mixed (retourne l'exception capturée)
    methods.insert("assertRaises".into(), FuncSig {
        params: vec![("callable".to_string(), Type::Mixed)],
        ret_ty: Type::Mixed,
        is_static: true,
        is_async: false,
        has_variadic: false,
        fixed_params_count: 1,
        required_params_count: 1,
        message_emit_in_loop: false,
    });

    // UnitTest::assertExceptionMessageEquals(message, expected) → void
    methods.insert("assertExceptionMessageEquals".into(), m(
        vec![("message", Type::String), ("expected", Type::String)],
        Type::Void,
    ));

    // UnitTest::assertExceptionMessageNotEquals(message, expected) → void
    methods.insert("assertExceptionMessageNotEquals".into(), m(
        vec![("message", Type::String), ("expected", Type::String)],
        Type::Void,
    ));

    // UnitTest::assertExceptionCodeEquals(code, expected) → void
    methods.insert("assertExceptionCodeEquals".into(), m(
        vec![("code", Type::Int), ("expected", Type::Int)],
        Type::Void,
    ));

    // UnitTest::assertExceptionCodeNotEquals(code, expected) → void
    methods.insert("assertExceptionCodeNotEquals".into(), m(
        vec![("code", Type::Int), ("expected", Type::Int)],
        Type::Void,
    ));

    // UnitTest::assertExceptionSourceEquals(source, expected) → void
    methods.insert("assertExceptionSourceEquals".into(), m(
        vec![("source", Type::String), ("expected", Type::String)],
        Type::Void,
    ));

    // UnitTest::assertExceptionSourceNotEquals(source, expected) → void
    methods.insert("assertExceptionSourceNotEquals".into(), m(
        vec![("source", Type::String), ("expected", Type::String)],
        Type::Void,
    ));

    // Résultat du flot runtime d'un RuntimeTest.oc (voir runtime/src/unittest_runtime.rs).
    methods.insert("assertSuccessResult".into(), m(vec![], Type::Void));
    methods.insert("assertErrorResult".into(), m(vec![], Type::Void));
    methods.insert("assertErrorResultCode".into(), m(vec![("code", Type::Int)], Type::Void));

    // Expression analysée, jamais évaluée : remplacée par `__staticAssert`
    // (voir src/sema/blocking_assert.rs).
    methods.insert("assertBlocksMainThread".into(), m(vec![("expr", Type::Mixed)], Type::Void));
    methods.insert("assertIsBlocking".into(), m(vec![("expr", Type::Mixed)], Type::Void));

    methods.insert("__recordRuntimeResult".into(), m(vec![("code", Type::Int)], Type::Void));
    methods.insert("__reportFailure".into(), m(vec![("label", Type::String), ("message", Type::String)], Type::Void));
    methods.insert("__staticAssert".into(), m(
        vec![("ok", Type::Bool), ("message", Type::String), ("code", Type::Int)],
        Type::Void,
    ));

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}
