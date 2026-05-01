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
//
// Convention runtime : UnitTest_<method>
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::ast::Type;
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

    ClassInfo {
        extends:      None,
        implements:   vec![],
        fields:       HashMap::new(),
        methods,
        class_consts: HashMap::new(),
        is_opaque:    false,
    }
}
