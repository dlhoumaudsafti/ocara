//! Points d'entrée du runtime Ocara en ABI C (runtime/src/native.rs,
//! runtime/src/exception.rs, runtime/src/lib.rs).

unsafe extern "C" {
    fn __str_byte_len(val: i64) -> i64;
    fn __str_from_utf8(ptr: *const u8, len: usize) -> i64;
    fn __raise_builtin_exception(
        class: *const u8, class_len: usize,
        source: *const u8, source_len: usize,
        message: *const u8, message_len: usize,
        code: i64,
    ) -> !;
    fn __array_new() -> i64;
    fn __array_push_owned(array: i64, val: i64);
    fn __rc_mark_raw_kind(val: i64, raw_kind: i64) -> i64;
}

const RAW_FLOAT: i64 = 2;
const MESSAGE_MAX: usize = 1024;

/// Texte d'une chaîne Ocara, vide pour `null`.
pub fn str_arg<'a>(val: i64) -> &'a str {
    if val == 0 {
        return "";
    }
    unsafe {
        let bytes = std::slice::from_raw_parts(val as *const u8, __str_byte_len(val) as usize);
        std::str::from_utf8_unchecked(bytes)
    }
}

/// Chaîne Ocara possédée.
pub fn string(text: &str) -> i64 {
    unsafe { __str_from_utf8(text.as_ptr(), text.len()) }
}

pub fn float_array(values: &[f64]) -> i64 {
    unsafe {
        let array = __rc_mark_raw_kind(__array_new(), RAW_FLOAT);
        for v in values {
            __array_push_owned(array, v.to_bits() as i64);
        }
        array
    }
}

pub fn string_array(values: &[String]) -> i64 {
    unsafe {
        let array = __array_new();
        for v in values {
            __array_push_owned(array, string(v));
        }
        array
    }
}

/// Lève `class`. Le `longjmp` saute tout destructeur : le message est
/// recopié sur la pile (tronqué à `MESSAGE_MAX` octets) et libéré avant.
pub fn raise(class: &'static str, source: &'static str, message: String, code: i64) -> ! {
    let mut len = message.len().min(MESSAGE_MAX);
    while !message.is_char_boundary(len) {
        len -= 1;
    }
    let mut buffer = [0u8; MESSAGE_MAX];
    buffer[..len].copy_from_slice(&message.as_bytes()[..len]);
    drop(message);
    unsafe {
        __raise_builtin_exception(
            class.as_ptr(), class.len(),
            source.as_ptr(), source.len(),
            buffer.as_ptr(), len,
            code,
        )
    }
}
