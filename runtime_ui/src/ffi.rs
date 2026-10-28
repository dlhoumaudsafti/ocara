//! Runtime Ocara en ABI C (runtime/src/native.rs, exception.rs, lib.rs,
//! rc.rs) : chaînes, valeurs `mixed` ↔ JSON, exceptions, closures.

use serde_json::Value;

unsafe extern "C" {
    fn __str_byte_len(val: i64) -> i64;
    fn __str_from_utf8(ptr: *const u8, len: usize) -> i64;
    fn __raise_builtin_exception(
        class: *const u8, class_len: usize,
        source: *const u8, source_len: usize,
        message: *const u8, message_len: usize,
        code: i64,
    ) -> !;
    fn JSON_encode(data: i64, leaf_kind: i64) -> i64;
    fn JSON_decode(json: i64) -> i64;
    pub(crate) fn __rc_retain(val: i64);
    pub(crate) fn __rc_release(val: i64);
    pub(crate) fn __rc_thread_enter();
    pub(crate) fn __rc_thread_leave();
    pub(crate) fn __native_new(data: i64, drop_fn: i64) -> i64;
    pub(crate) fn __native_data(val: i64, drop_fn: i64) -> i64;
    pub(crate) fn __call_closure2_catching(fat_ptr: i64, arg1: i64, arg2: i64) -> i64;
}

const MESSAGE_MAX: usize = 1024;

/// Texte d'une chaîne Ocara, vide pour `null`.
pub fn str_arg(val: i64) -> String {
    if val == 0 {
        return String::new();
    }
    unsafe {
        let bytes = std::slice::from_raw_parts(val as *const u8, __str_byte_len(val) as usize);
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Chaîne Ocara possédée.
pub fn string(text: &str) -> i64 {
    unsafe { __str_from_utf8(text.as_ptr(), text.len()) }
}

/// Valeur Ocara (`mixed`, tableau, map…) en JSON.
pub fn to_json(val: i64) -> Value {
    if val == 0 {
        return Value::Null;
    }
    unsafe {
        let encoded = JSON_encode(val, 0);
        let json = serde_json::from_str(&str_arg(encoded)).unwrap_or(Value::Null);
        __rc_release(encoded);
        json
    }
}

/// Valeur JSON en valeur Ocara possédée (`null` → 0).
pub fn from_json(value: &Value) -> i64 {
    match value {
        Value::Null => 0,
        Value::String(text) => string(text),
        other => unsafe {
            let text = string(&other.to_string());
            let val = JSON_decode(text);
            __rc_release(text);
            val
        },
    }
}

/// Lève `UIException` (`code`, `message`). Le `longjmp` saute tout
/// destructeur : le message est recopié sur la pile et libéré avant.
pub fn raise(code: i64, message: String) -> ! {
    let (class, source) = ("UIException", "UI");
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
