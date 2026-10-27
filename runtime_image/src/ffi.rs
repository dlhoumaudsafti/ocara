// Points d'entrée du runtime principal, en ABI C (runtime/src/native.rs,
// runtime/src/exception.rs, runtime/src/lib.rs).

unsafe extern "C" {
    fn __native_new(data: i64, drop_fn: i64) -> i64;
    fn __native_data(val: i64, drop_fn: i64) -> i64;
    fn __str_byte_len(val: i64) -> i64;
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

const RAW_INT: i64 = 1;

/// Valeur Ocara comptée qui possède `value`, détruite par `drop_fn`.
pub(crate) fn native_new<T>(value: T, drop_fn: extern "C" fn(i64)) -> i64 {
    let data = Box::into_raw(Box::new(value)) as i64;
    unsafe { __native_new(data, drop_fn as usize as i64) }
}

/// Structure d'une valeur créée par `native_new` avec le même `drop_fn`.
pub(crate) fn native_ref<'a, T>(val: i64, drop_fn: extern "C" fn(i64)) -> Option<&'a T> {
    let data = unsafe { __native_data(val, drop_fn as usize as i64) };
    (data != 0).then(|| unsafe { &*(data as *const T) })
}

/// Libère une structure de `native_new` (corps d'un `drop_fn`).
pub(crate) unsafe fn native_drop<T>(data: i64) {
    drop(unsafe { Box::from_raw(data as *mut T) });
}

pub(crate) fn str_arg<'a>(val: i64) -> &'a str {
    if val == 0 {
        return "";
    }
    unsafe {
        let bytes = std::slice::from_raw_parts(val as *const u8, __str_byte_len(val) as usize);
        std::str::from_utf8_unchecked(bytes)
    }
}

const MESSAGE_MAX: usize = 1024;

/// Lève `class`. Le `longjmp` saute tout destructeur : le message est
/// recopié sur la pile (tronqué à `MESSAGE_MAX` octets) et libéré avant.
pub(crate) fn raise(class: &'static str, source: &'static str, message: String, code: i64) -> ! {
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

pub(crate) fn int_array(values: &[i64]) -> i64 {
    unsafe {
        let array = __rc_mark_raw_kind(__array_new(), RAW_INT);
        for &v in values {
            __array_push_owned(array, v);
        }
        array
    }
}
