// Valeurs natives comptées (`TAG_NATIVE`) : donnée détruite avec le bloc,
// accès réservé au type qui l'a créée.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::native::*;
use crate::rc::*;
use crate::*;

static DROPS: AtomicUsize = AtomicUsize::new(0);

extern "C" fn drop_probe(data: i64) {
    assert_eq!(data, 41);
    DROPS.fetch_add(1, Ordering::SeqCst);
}

extern "C" fn drop_other(_data: i64) {}

#[test]
fn native_value_is_dropped_with_last_reference() {
    let v = __native_new(41, drop_probe as *const () as i64);
    assert_eq!(__rc_count(v), 1);
    __rc_retain(v);
    __rc_release(v);
    assert_eq!(DROPS.load(Ordering::SeqCst), 0);
    __rc_release(v);
    assert_eq!(DROPS.load(Ordering::SeqCst), 1);
}

#[test]
fn native_data_checks_the_type() {
    let v = __native_new(7, drop_other as *const () as i64);
    assert_eq!(__native_data(v, drop_other as *const () as i64), 7);
    assert_eq!(__native_data(v, drop_probe as *const () as i64), 0);
    assert_eq!(__native_data(0, drop_other as *const () as i64), 0);
    let s = unsafe { alloc_str("texte") };
    assert_eq!(__native_data(s, drop_other as *const () as i64), 0);
    assert_eq!(__str_byte_len(s), 5);
    assert_eq!(__str_byte_len(0), 0);
    __rc_release(s);
    __rc_release(v);
}
