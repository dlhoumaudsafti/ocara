// Accès par clé à une valeur `mixed` (mixed_index.rs) : map ou tableau selon
// l'étiquette du bloc, éléments bruts boxés à la lecture et déboxés à
// l'écriture.

use crate::mixed_index::{Convert_mixedGet, Convert_mixedSet};
use crate::*;

#[test]
fn map_key_is_read_and_written_through_mixed() {
    let m = __map_new();
    let key = unsafe { alloc_str("user") };
    let bob = unsafe { alloc_str("bob") };
    Convert_mixedSet(m, key, bob);
    let read = Convert_mixedGet(m, key);
    assert_eq!(unsafe { ptr_to_str(read) }, "bob");
    unsafe {
        rc::release(read);
        rc::release(bob);
        rc::release(key);
        rc::release(m);
    }
}

#[test]
fn raw_int_elements_are_boxed_and_unboxed() {
    let m = rc::__rc_mark_raw_kind(__map_new(), 1);
    let key = unsafe { alloc_str("big") };
    let big = 9_000_000_000i64;
    Convert_mixedSet(m, key, box_int_if_needed(big));
    assert_eq!(__map_get(m, key), big);
    let read = Convert_mixedGet(m, key);
    assert_eq!(__mixed_to_int(read), big);
    unsafe {
        rc::release(read);
        rc::release(key);
        rc::release(m);
    }
}

#[test]
fn array_is_indexed_by_a_raw_or_boxed_int() {
    let a = rc::__rc_mark_raw_kind(__array_new(), 1);
    __array_push(a, 10);
    __array_push(a, 20);
    assert_eq!(__mixed_to_int(Convert_mixedGet(a, 1)), 20);
    Convert_mixedSet(a, box_int_if_needed(0), 7);
    assert_eq!(__array_get(a, 0), 7);
    unsafe { rc::release(a) };
}

#[test]
fn null_reads_null_and_ignores_writes() {
    assert_eq!(Convert_mixedGet(0, 1), 0);
    Convert_mixedSet(0, 1, 2);
}
