/// Builtins déclarés côté Ocara runtime.
///
/// Ces fonctions sont résolues à la liaison (link) depuis la bibliothèque
/// runtime C minimale `ocara_runtime.c` (ou implémentées inline via Cranelift).
///
/// Chaque entry décrit la signature attendue par Cranelift.
use cranelift_codegen::ir::{AbiParam, Signature};
use cranelift_codegen::isa::CallConv;
use cranelift_codegen::ir::types as clt;

pub struct BuiltinDesc {
    pub name:    &'static str,
    pub params:  &'static [cranelift_codegen::ir::Type],
    pub returns: Option<cranelift_codegen::ir::Type>,
    /// Module ocara requis pour utiliser ce builtin (ex: "Array", "IO").
    /// `None` = interne (toujours disponible, pas d'import requis).
    pub module:  Option<&'static str>,
}

pub const BUILTINS: &[BuiltinDesc] = &[
    // ── Internes (toujours disponibles, jamais appelables directement) ────────
    BuiltinDesc { name: "__str_concat",      params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__val_to_str",      params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__str_from_float",  params: &[clt::F64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__str_from_bool",   params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__box_float",       params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__box_bool",        params: &[clt::I64],                             returns: Some(clt::I64),    module: None },

    BuiltinDesc { name: "__range",           params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__array_new",       params: &[],                                     returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__array_push",      params: &[clt::I64, clt::I64],                   returns: None,              module: None },
    BuiltinDesc { name: "__array_len",       params: &[clt::I64],                             returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__array_get",       params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__array_set",       params: &[clt::I64, clt::I64, clt::I64],         returns: None,              module: None },
    BuiltinDesc { name: "__map_foreach",     params: &[clt::I64, clt::I64, clt::I64],         returns: None,              module: None },
    BuiltinDesc { name: "__map_new",         params: &[],                                     returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__map_set",         params: &[clt::I64, clt::I64, clt::I64],         returns: None,              module: None },
    BuiltinDesc { name: "__map_get",         params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: None },
    // ── ocara.String ─────────────────────────────────────────────────────────
    BuiltinDesc { name: "String_len",        params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("String") },
    BuiltinDesc { name: "String_upper",      params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("String") },
    BuiltinDesc { name: "String_lower",      params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("String") },
    BuiltinDesc { name: "String_capitalize", params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("String") },
    BuiltinDesc { name: "String_trim",       params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("String") },
    BuiltinDesc { name: "String_replace",    params: &[clt::I64, clt::I64, clt::I64],         returns: Some(clt::I64),    module: Some("String") },
    BuiltinDesc { name: "String_split",      params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("String") },
    BuiltinDesc { name: "String_explode",    params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("String") },
    BuiltinDesc { name: "String_between",    params: &[clt::I64, clt::I64, clt::I64],         returns: Some(clt::I64),    module: Some("String") },
    BuiltinDesc { name: "String_empty",      params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("String") },
    // ── ocara.Math ───────────────────────────────────────────────────────────
    BuiltinDesc { name: "Math_abs",          params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("Math") },
    BuiltinDesc { name: "Math_min",          params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("Math") },
    BuiltinDesc { name: "Math_max",          params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("Math") },
    BuiltinDesc { name: "Math_pow",          params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("Math") },
    BuiltinDesc { name: "Math_clamp",        params: &[clt::I64, clt::I64, clt::I64],         returns: Some(clt::I64),    module: Some("Math") },
    BuiltinDesc { name: "Math_sqrt",         params: &[clt::F64],                             returns: Some(clt::F64),    module: Some("Math") },
    BuiltinDesc { name: "Math_floor",        params: &[clt::F64],                             returns: Some(clt::I64),    module: Some("Math") },
    BuiltinDesc { name: "Math_ceil",         params: &[clt::F64],                             returns: Some(clt::I64),    module: Some("Math") },
    BuiltinDesc { name: "Math_round",        params: &[clt::F64],                             returns: Some(clt::I64),    module: Some("Math") },
    // ── ocara.Regex ──────────────────────────────────────────────────────────
    BuiltinDesc { name: "Regex_test",        params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("Regex") },
    BuiltinDesc { name: "Regex_find",        params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("Regex") },
    BuiltinDesc { name: "Regex_find_all",    params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("Regex") },
    BuiltinDesc { name: "Regex_replace",     params: &[clt::I64, clt::I64, clt::I64],         returns: Some(clt::I64),    module: Some("Regex") },
    BuiltinDesc { name: "Regex_replace_all", params: &[clt::I64, clt::I64, clt::I64],         returns: Some(clt::I64),    module: Some("Regex") },
    BuiltinDesc { name: "Regex_split",       params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("Regex") },
    BuiltinDesc { name: "Regex_count",       params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("Regex") },
    BuiltinDesc { name: "Regex_extract",     params: &[clt::I64, clt::I64, clt::I64],         returns: Some(clt::I64),    module: Some("Regex") },
    // ── ocara.Array ──────────────────────────────────────────────────────────
    BuiltinDesc { name: "Array_len",         params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("Array") },
    BuiltinDesc { name: "Array_push",        params: &[clt::I64, clt::I64],                   returns: None,              module: Some("Array") },
    BuiltinDesc { name: "Array_pop",         params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("Array") },
    BuiltinDesc { name: "Array_first",       params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("Array") },
    BuiltinDesc { name: "Array_last",        params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("Array") },
    BuiltinDesc { name: "Array_contains",    params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("Array") },
    BuiltinDesc { name: "Array_index_of",    params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("Array") },
    BuiltinDesc { name: "Array_reverse",     params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("Array") },
    BuiltinDesc { name: "Array_slice",       params: &[clt::I64, clt::I64, clt::I64],         returns: Some(clt::I64),    module: Some("Array") },
    BuiltinDesc { name: "Array_join",        params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("Array") },
    BuiltinDesc { name: "Array_sort",        params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("Array") },
    // ── ocara.Map ────────────────────────────────────────────────────────────
    BuiltinDesc { name: "Map_size",          params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("Map") },
    BuiltinDesc { name: "Map_has",           params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("Map") },
    BuiltinDesc { name: "Map_get",           params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("Map") },
    BuiltinDesc { name: "Map_set",           params: &[clt::I64, clt::I64, clt::I64],         returns: None,              module: Some("Map") },
    BuiltinDesc { name: "Map_remove",        params: &[clt::I64, clt::I64],                   returns: None,              module: Some("Map") },
    BuiltinDesc { name: "Map_keys",          params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("Map") },
    BuiltinDesc { name: "Map_values",        params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("Map") },
    BuiltinDesc { name: "Map_merge",         params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("Map") },
    BuiltinDesc { name: "Map_is_empty",      params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("Map") },
    // ── ocara.IO ─────────────────────────────────────────────────────────────
    BuiltinDesc { name: "IO_write",          params: &[clt::I64],                             returns: None,              module: Some("IO") },
    BuiltinDesc { name: "IO_write_int",      params: &[clt::I64],                             returns: None,              module: Some("IO") },
    BuiltinDesc { name: "IO_write_float",    params: &[clt::F64],                             returns: None,              module: Some("IO") },
    BuiltinDesc { name: "IO_write_bool",     params: &[clt::I64],                             returns: None,              module: Some("IO") },
    BuiltinDesc { name: "IO_writeln",        params: &[clt::I64],                             returns: None,              module: Some("IO") },
    BuiltinDesc { name: "IO_writeln_int",    params: &[clt::I64],                             returns: None,              module: Some("IO") },
    BuiltinDesc { name: "IO_writeln_float",  params: &[clt::F64],                             returns: None,              module: Some("IO") },
    BuiltinDesc { name: "IO_writeln_bool",   params: &[clt::I64],                             returns: None,              module: Some("IO") },
    BuiltinDesc { name: "IO_read",           params: &[],                                     returns: Some(clt::I64),    module: Some("IO") },
    BuiltinDesc { name: "IO_readln",         params: &[],                                     returns: Some(clt::I64),    module: Some("IO") },
    BuiltinDesc { name: "IO_read_int",       params: &[],                                     returns: Some(clt::I64),    module: Some("IO") },
    BuiltinDesc { name: "IO_read_float",     params: &[],                                     returns: Some(clt::F64),    module: Some("IO") },
    BuiltinDesc { name: "IO_read_bool",      params: &[],                                     returns: Some(clt::I64),    module: Some("IO") },
    BuiltinDesc { name: "IO_read_array",     params: &[clt::I64],                             returns: Some(clt::I64),    module: Some("IO") },
    BuiltinDesc { name: "IO_read_map",       params: &[clt::I64, clt::I64],                   returns: Some(clt::I64),    module: Some("IO") },
    // ── ocara.Convert ────────────────────────────────────────────────────────
    BuiltinDesc { name: "Convert_str_to_int",          params: &[clt::I64],                   returns: Some(clt::I64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_str_to_float",        params: &[clt::I64],                   returns: Some(clt::F64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_str_to_bool",         params: &[clt::I64],                   returns: Some(clt::I64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_str_to_array",        params: &[clt::I64, clt::I64],         returns: Some(clt::I64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_str_to_map",          params: &[clt::I64, clt::I64, clt::I64], returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_int_to_str",          params: &[clt::I64],                   returns: Some(clt::I64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_int_to_float",        params: &[clt::I64],                   returns: Some(clt::F64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_int_to_bool",         params: &[clt::I64],                   returns: Some(clt::I64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_float_to_str",        params: &[clt::F64],                   returns: Some(clt::I64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_float_to_int",        params: &[clt::F64],                   returns: Some(clt::I64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_float_to_bool",       params: &[clt::F64],                   returns: Some(clt::I64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_bool_to_str",         params: &[clt::I64],                   returns: Some(clt::I64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_bool_to_int",         params: &[clt::I64],                   returns: Some(clt::I64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_bool_to_float",       params: &[clt::I64],                   returns: Some(clt::F64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_array_to_str",        params: &[clt::I64, clt::I64],         returns: Some(clt::I64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_array_to_map",        params: &[clt::I64, clt::I64],         returns: Some(clt::I64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_map_to_str",          params: &[clt::I64, clt::I64, clt::I64], returns: Some(clt::I64), module: Some("Convert") },
    BuiltinDesc { name: "Convert_map_keys_to_array",   params: &[clt::I64],                   returns: Some(clt::I64),    module: Some("Convert") },
    BuiltinDesc { name: "Convert_map_values_to_array", params: &[clt::I64],                   returns: Some(clt::I64),    module: Some("Convert") },
    // ── ocara.HTTPRequest ─────────────────────────────────────────────────────
    BuiltinDesc { name: "HTTPRequest_new",         params: &[clt::I64],                       returns: Some(clt::I64),    module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_set_method",  params: &[clt::I64, clt::I64],             returns: None,              module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_set_header",  params: &[clt::I64, clt::I64, clt::I64],   returns: None,              module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_set_body",    params: &[clt::I64, clt::I64],             returns: None,              module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_set_timeout", params: &[clt::I64, clt::I64],             returns: None,              module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_send",        params: &[clt::I64],                       returns: Some(clt::I64),    module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_status",      params: &[clt::I64],                       returns: Some(clt::I64),    module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_body",        params: &[clt::I64],                       returns: Some(clt::I64),    module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_header",      params: &[clt::I64, clt::I64],             returns: Some(clt::I64),    module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_headers",     params: &[clt::I64],                       returns: Some(clt::I64),    module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_ok",          params: &[clt::I64],                       returns: Some(clt::I64),    module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_is_error",    params: &[clt::I64],                       returns: Some(clt::I64),    module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_error",       params: &[clt::I64],                       returns: Some(clt::I64),    module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_get",         params: &[clt::I64],                       returns: Some(clt::I64),    module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_post",        params: &[clt::I64, clt::I64],             returns: Some(clt::I64),    module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_put",         params: &[clt::I64, clt::I64],             returns: Some(clt::I64),    module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_delete",      params: &[clt::I64],                       returns: Some(clt::I64),    module: Some("HTTPRequest") },
    BuiltinDesc { name: "HTTPRequest_patch",       params: &[clt::I64, clt::I64],             returns: Some(clt::I64),    module: Some("HTTPRequest") },
    // ── ocara.System ─────────────────────────────────────────────────────────
    BuiltinDesc { name: "System_exec",        params: &[clt::I64],                            returns: Some(clt::I64),    module: Some("System") },
    BuiltinDesc { name: "System_passthrough", params: &[clt::I64],                            returns: Some(clt::I64),    module: Some("System") },
    BuiltinDesc { name: "System_exec_code",   params: &[clt::I64],                            returns: Some(clt::I64),    module: Some("System") },
    BuiltinDesc { name: "System_exit",        params: &[clt::I64],                            returns: None,              module: Some("System") },    BuiltinDesc { name: "System_env",         params: &[clt::I64],                            returns: Some(clt::I64),    module: Some("System") },
    BuiltinDesc { name: "System_set_env",     params: &[clt::I64, clt::I64],                  returns: None,              module: Some("System") },
    BuiltinDesc { name: "System_cwd",         params: &[],                                    returns: Some(clt::I64),    module: Some("System") },
    BuiltinDesc { name: "System_sleep",       params: &[clt::I64],                            returns: None,              module: Some("System") },
    BuiltinDesc { name: "System_pid",         params: &[],                                    returns: Some(clt::I64),    module: Some("System") },
    BuiltinDesc { name: "System_args",        params: &[],                                    returns: Some(clt::I64),    module: Some("System") },
    // ── ocara.HTTPServer ───────────────────────────────────────────────────────
    BuiltinDesc { name: "HTTPServer_init",           params: &[clt::I64],                            returns: None,              module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_set_port",        params: &[clt::I64, clt::I64],                  returns: None,              module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_set_host",        params: &[clt::I64, clt::I64],                  returns: None,              module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_set_workers",     params: &[clt::I64, clt::I64],                  returns: None,              module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_route",           params: &[clt::I64, clt::I64, clt::I64, clt::I64], returns: None,           module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_run",             params: &[clt::I64],                            returns: None,              module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_req_path",        params: &[clt::I64],                            returns: Some(clt::I64),    module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_req_method",      params: &[clt::I64],                            returns: Some(clt::I64),    module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_req_body",        params: &[clt::I64],                            returns: Some(clt::I64),    module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_req_header",      params: &[clt::I64, clt::I64],                  returns: Some(clt::I64),    module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_req_query",       params: &[clt::I64, clt::I64],                  returns: Some(clt::I64),    module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_respond",         params: &[clt::I64, clt::I64, clt::I64],        returns: None,              module: Some("HTTPServer") },
    BuiltinDesc { name: "HTTPServer_set_resp_header", params: &[clt::I64, clt::I64, clt::I64],        returns: None,              module: Some("HTTPServer") },
    // ── ocara.Thread ─────────────────────────────────────────────────────────
    BuiltinDesc { name: "Thread_init",        params: &[clt::I64],                            returns: None,              module: Some("Thread") },
    BuiltinDesc { name: "Thread_run",         params: &[clt::I64, clt::I64],                  returns: None,              module: Some("Thread") },
    BuiltinDesc { name: "Thread_join",        params: &[clt::I64],                            returns: None,              module: Some("Thread") },
    BuiltinDesc { name: "Thread_detach",      params: &[clt::I64],                            returns: None,              module: Some("Thread") },
    BuiltinDesc { name: "Thread_id",          params: &[clt::I64],                            returns: Some(clt::I64),    module: Some("Thread") },
    BuiltinDesc { name: "Thread_sleep",       params: &[clt::I64],                            returns: None,              module: Some("Thread") },
    BuiltinDesc { name: "Thread_current_id",  params: &[],                                    returns: Some(clt::I64),    module: Some("Thread") },
    // ── ocara.UnitTest ───────────────────────────────────────────────────────
    BuiltinDesc { name: "UnitTest_assertEquals",         params: &[clt::I64, clt::I64],       returns: None,              module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertNotEquals",      params: &[clt::I64, clt::I64],       returns: None,              module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertTrue",           params: &[clt::I64],                 returns: None,              module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertFalse",          params: &[clt::I64],                 returns: None,              module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertNull",           params: &[clt::I64],                 returns: None,              module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertNotNull",        params: &[clt::I64],                 returns: None,              module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertGreater",        params: &[clt::I64, clt::I64],       returns: None,              module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertLess",           params: &[clt::I64, clt::I64],       returns: None,              module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertGreaterOrEquals",params: &[clt::I64, clt::I64],       returns: None,              module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertLessOrEquals",   params: &[clt::I64, clt::I64],       returns: None,              module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertContains",       params: &[clt::I64, clt::I64],       returns: None,              module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertEmpty",          params: &[clt::I64],                 returns: None,              module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_assertNotEmpty",       params: &[clt::I64],                 returns: None,              module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_fail",                 params: &[clt::I64],                 returns: None,              module: Some("UnitTest") },
    BuiltinDesc { name: "UnitTest_pass",                 params: &[clt::I64],                 returns: None,              module: Some("UnitTest") },
    // ── Gestion des erreurs (try/on/fail) — toujours disponibles ─────────────
    BuiltinDesc { name: "__ocara_try_exec",       params: &[clt::I64, clt::I64],              returns: None,              module: None },
    BuiltinDesc { name: "__ocara_fail",           params: &[clt::I64, clt::I64],              returns: None,              module: None },
    BuiltinDesc { name: "__ocara_type_matches",   params: &[clt::I64, clt::I64],              returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__ocara_unhandled_fail", params: &[clt::I64],                        returns: None,              module: None },
    // Allocation d'objet tas (toujours disponible)
    BuiltinDesc { name: "__alloc_obj",            params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    // Conversion string — sans heuristique pointeur
    BuiltinDesc { name: "__str_from_int",         params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__array_to_str",         params: &[clt::I64],                        returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__system_os",            params: &[],                                returns: Some(clt::I64),    module: None },
    BuiltinDesc { name: "__system_arch",          params: &[],                                returns: Some(clt::I64),    module: None },
];

pub fn builtin_sig(desc: &BuiltinDesc, call_conv: CallConv) -> Signature {
    let mut sig = Signature::new(call_conv);
    for &ty in desc.params {
        sig.params.push(AbiParam::new(ty));
    }
    if let Some(ret) = desc.returns {
        sig.returns.push(AbiParam::new(ret));
    }
    sig
}
