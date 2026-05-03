use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module UnitTest
pub const UNITTEST_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "UnitTest_assertEquals",                    params: &[clt::I64, clt::I64], returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertNotEquals",                 params: &[clt::I64, clt::I64], returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertTrue",                      params: &[clt::I64],           returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertFalse",                     params: &[clt::I64],           returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertNull",                      params: &[clt::I64],           returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertNotNull",                   params: &[clt::I64],           returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertGreater",                   params: &[clt::I64, clt::I64], returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertLess",                      params: &[clt::I64, clt::I64], returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertGreaterOrEquals",           params: &[clt::I64, clt::I64], returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertLessOrEquals",              params: &[clt::I64, clt::I64], returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertContains",                  params: &[clt::I64, clt::I64], returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertEmpty",                     params: &[clt::I64],           returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertNotEmpty",                  params: &[clt::I64],           returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_fail",                            params: &[clt::I64],           returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_pass",                            params: &[clt::I64],           returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertFunction",                  params: &[clt::I64],           returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertClass",                     params: &[clt::I64],           returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertEnum",                      params: &[clt::I64],           returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertMap",                       params: &[clt::I64],           returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertArray",                     params: &[clt::I64],           returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertRaises",                    params: &[clt::I64],           returns: Some(clt::I64), module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertExceptionMessageEquals",    params: &[clt::I64, clt::I64], returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertExceptionMessageNotEquals", params: &[clt::I64, clt::I64], returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertExceptionCodeEquals",       params: &[clt::I64, clt::I64], returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertExceptionCodeNotEquals",    params: &[clt::I64, clt::I64], returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertExceptionSourceEquals",     params: &[clt::I64, clt::I64], returns: None,           module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertExceptionSourceNotEquals",  params: &[clt::I64, clt::I64], returns: None,           module: Some("UnitTest") },
];
