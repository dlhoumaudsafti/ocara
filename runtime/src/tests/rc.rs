// Comptage de références et détecteur de cycles
// (docs/roadmap.d/memoire-refcount.md, phase 1).

use std::sync::Mutex;

use crate::rc::*;
use crate::*;

/// La collecte des cycles suppose qu'aucun autre thread ne modifie de
/// comptes pendant qu'elle tourne : les tests de ce module sont sérialisés.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

fn owned(s: &str) -> i64 {
    unsafe { alloc_str(s) }
}

#[test]
fn uncounted_values_are_ignored() {
    let _g = serial();
    for v in [0i64, 1, 42, 65535] {
        assert_eq!(__rc_count(v), 0);
        __rc_retain(v);
        __rc_release(v);
    }
}

#[test]
fn string_count_follows_retain_and_release() {
    let _g = serial();
    let s = owned("abc");
    assert_eq!(__rc_count(s), 1);
    __rc_retain(s);
    assert_eq!(__rc_count(s), 2);
    __rc_release(s);
    assert_eq!(__rc_count(s), 1);
    assert_eq!(unsafe { ptr_to_str(s) }, "abc");
    __rc_release(s);
}

#[test]
fn boxed_values_are_counted() {
    let _g = serial();
    let b = __box_float(1.5f64.to_bits() as i64);
    assert_eq!(__rc_count(b), 1);
    __rc_retain(b);
    assert_eq!(__rc_count(b), 2);
    __rc_release(b);
    __rc_release(b);
}

#[test]
fn releasing_an_array_releases_its_elements() {
    let _g = serial();
    let s = owned("elem");
    let arr = __array_new();
    array_push_owned(arr, s);
    __rc_retain(s);
    assert_eq!(__rc_count(s), 2);
    __rc_release(arr);
    assert_eq!(__rc_count(s), 1);
    __rc_release(s);
}

#[test]
fn releasing_a_map_releases_its_values() {
    let _g = serial();
    let s = owned("value");
    let m = __map_new();
    map_set_owned_key(m, owned("k"), s);
    __rc_retain(s);
    __rc_release(m);
    assert_eq!(__rc_count(s), 1);
    __rc_release(s);
}

#[test]
fn raw_array_elements_are_never_followed() {
    let _g = serial();
    let arr = __rc_mark_raw(__array_new());
    for n in [0x10000i64, 0x20000, 0x7fff_0000, 0x1234_5678_0000] {
        array_push_owned(arr, n);
    }
    __rc_release(arr);
}

#[test]
fn object_releases_only_heap_fields() {
    let _g = serial();
    let obj = __alloc_class_obj(16, 1, owned("10"));
    let s = owned("field");
    unsafe {
        *(obj as *mut i64) = s;
        *((obj + 8) as *mut i64) = 0x30000;
    }
    __rc_retain(s);
    __rc_release(obj);
    assert_eq!(__rc_count(s), 1);
    __rc_release(s);
}

/// `a` contient `b`, `b` contient `a`, `a` contient aussi `sentinel`.
fn make_cycle(sentinel: i64) -> (i64, i64) {
    let a = __array_new();
    let b = __array_new();
    array_push_owned(a, b);
    array_push_owned(a, sentinel);
    __rc_retain(a);
    array_push_owned(b, a);
    (a, b)
}

#[test]
fn unreachable_cycle_is_collected() {
    let _g = serial();
    let sentinel = owned("sentinel");
    __rc_retain(sentinel);
    let (a, _b) = make_cycle(sentinel);
    __rc_release(a);
    assert_eq!(__rc_count(sentinel), 2);
    __rc_collect_cycles();
    assert_eq!(__rc_count(sentinel), 1);
    __rc_release(sentinel);
}

#[test]
fn reachable_cycle_survives_collection() {
    let _g = serial();
    let sentinel = owned("sentinel");
    __rc_retain(sentinel);
    let (a, b) = make_cycle(sentinel);
    __rc_retain(b);
    __rc_release(a);
    __rc_collect_cycles();
    assert_eq!(__rc_count(sentinel), 2);
    assert_eq!(__rc_count(a), 1);
    assert_eq!(__rc_count(b), 2);
    assert_eq!(__array_get(b, 0), a);
    __rc_release(b);
    __rc_collect_cycles();
    assert_eq!(__rc_count(sentinel), 1);
    __rc_release(sentinel);
}

#[test]
fn self_referencing_object_is_collected() {
    let _g = serial();
    let obj = __alloc_class_obj(16, 2, owned("11"));
    let sentinel = owned("sentinel");
    __rc_retain(sentinel);
    __rc_retain(obj);
    unsafe {
        *(obj as *mut i64) = obj;
        *((obj + 8) as *mut i64) = sentinel;
    }
    __rc_release(obj);
    __rc_collect_cycles();
    assert_eq!(__rc_count(sentinel), 1);
    __rc_release(sentinel);
}

#[test]
fn freed_while_buffered_is_reclaimed_by_collection() {
    let _g = serial();
    let sentinel = owned("sentinel");
    __rc_retain(sentinel);
    let arr = __array_new();
    array_push_owned(arr, sentinel);
    __rc_retain(arr);
    __rc_release(arr);
    __rc_release(arr);
    assert_eq!(__rc_count(sentinel), 1);
    __rc_collect_cycles();
    __rc_release(sentinel);
}

#[test]
fn long_chain_is_released_without_recursion() {
    let _g = serial();
    let head = __array_new();
    let mut cur = head;
    for _ in 0..200_000 {
        let next = __array_new();
        array_push_owned(cur, next);
        cur = next;
    }
    __rc_release(head);
}
