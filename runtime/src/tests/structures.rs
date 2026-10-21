// Map à index de hachage et concaténation en place
// (docs/roadmap.d/perf-runtime-complexite.md).

use crate::rc::*;
use crate::str_append::__str_append;
use crate::*;

fn key(s: &str) -> i64 {
    unsafe { alloc_str(s) }
}

#[test]
fn map_keeps_insertion_order_after_update_and_remove() {
    let m = __map_new();
    for k in ["b", "a", "c"] {
        let text = key(k);
        __map_set(m, text, 1);
        __rc_release(text);
    }
    let a = key("a");
    __map_set(m, a, 2);
    assert_eq!(__map_get(m, a), 2);
    Map_remove(m, a);
    assert_eq!(Map_has(m, a), 0);
    __rc_release(a);
    let keys = Map_keys(m, 0);
    let names: Vec<String> = (0..__array_len(keys)).map(|i| unsafe { ptr_to_str(__array_get(keys, i)).to_string() }).collect();
    assert_eq!(names, vec!["b", "c"]);
    __rc_release(keys);
    __rc_release(m);
}

#[test]
fn map_finds_int_keys_by_value() {
    let m = __map_new();
    for i in 0..1000 {
        __map_set(m, i, i * 2);
    }
    assert_eq!(__map_get(m, 999), 1998);
    assert_eq!(Map_size(m), 1000);
    __rc_release(m);
}

#[test]
fn append_grows_an_unshared_string_in_place() {
    let tail = key("x");
    let mut s = key("");
    s = __str_append(s, tail);
    let first = s;
    for _ in 0..20 {
        s = __str_append(s, tail);
    }
    assert_eq!(unsafe { ptr_to_str(s) }, "x".repeat(21));
    assert_eq!(s, first, "allongée en place dans sa capacité");
    __rc_release(s);
    __rc_release(tail);
}

#[test]
fn append_never_mutates_a_shared_string() {
    let tail = key("!");
    let s = __str_append(key("ab"), tail);
    __rc_retain(s);
    let other = __str_append(s, tail);
    assert_ne!(other, s);
    assert_eq!(unsafe { ptr_to_str(s) }, "ab!");
    assert_eq!(unsafe { ptr_to_str(other) }, "ab!!");
    __rc_release(s);
    __rc_release(other);
    __rc_release(tail);
}
