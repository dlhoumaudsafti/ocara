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

#[test]
fn small_blocks_are_reused_and_resized() {
    use crate::alloc_cache::{alloc, dealloc};
    use std::alloc::Layout;
    let layout = Layout::from_size_align(40, 8).unwrap();
    unsafe {
        let a = alloc(layout);
        dealloc(a, layout);
        let b = alloc(layout);
        assert_eq!(a, b, "bloc resservi par la liste du thread");
        dealloc(b, layout);
    }
    let mut v: Vec<u64> = Vec::new();
    for i in 0..1000 {
        v.push(i);
    }
    assert_eq!(v.iter().sum::<u64>(), 499500);
}

#[test]
fn int_text_formats_every_range() {
    for n in [0i64, 7, -7, 10, 99, 100, 101, 1005, -1000, 1234567890, i64::MAX, i64::MIN] {
        assert_eq!(int_text(n, &mut [0; 20]), n.to_string());
    }
}

#[test]
fn replace_and_split_keep_their_results() {
    unsafe {
        let r = String_replace(key("aXbXXc"), key("X"), key("--"));
        assert_eq!(ptr_to_str(r), "a--b----c");
        let r = String_replace(key("éàé"), key("é"), key("e"));
        assert_eq!(ptr_to_str(r), "eàe");
        let parts = String_split(key("a,,b,é"), key(","));
        let texts: Vec<String> = (0..__array_len(parts)).map(|i| ptr_to_str(__array_get(parts, i)).to_string()).collect();
        assert_eq!(texts, vec!["a", "", "b", "é"]);
        let parts = String_split(key("a::b"), key("::"));
        assert_eq!(__array_len(parts), 2);
        assert_eq!(String_len(key("éa")), 2);
        assert_eq!(ptr_to_str(String_upper(key("abé"))), "ABÉ");
    }
}

#[test]
fn template_parts_are_formatted_by_kind() {
    let text = key("x");
    let parts: [i64; 10] = [0, text, 1, -42, 2, 2.5f64.to_bits() as i64, 3, 1, 0, 0];
    let built = crate::str_append::__str_build(parts.as_ptr(), 5);
    assert_eq!(unsafe { ptr_to_str(built) }, "x-422.5truenull");
    __rc_release(built);
    __rc_release(text);
}

/// Clé `int` brute (`__map_set_ikey`/`__map_get_ikey`) et clé texte : même
/// clé quand le texte est le décimal canonique de l'entier (hachage par la
/// valeur, voir `containers::KeyState`), clés distinctes sinon (`"07"`).
#[test]
fn raw_int_keys_and_their_decimal_text_are_the_same_key() {
    let m = __map_new();
    for i in [-12i64, -1, 0, 7, 42, i64::MAX, i64::MIN] {
        __map_set_ikey(m, i, i.wrapping_add(1));
    }
    for i in [-12i64, -1, 0, 7, 42, i64::MAX, i64::MIN] {
        let text = key(&i.to_string());
        assert_eq!(__map_get(m, text), i.wrapping_add(1), "clé {}", i);
        __rc_release(text);
        assert_eq!(__map_get_ikey(m, i), i.wrapping_add(1));
    }
    let padded = key("07");
    assert_eq!(__map_get(m, padded), 0, "\"07\" n'est pas la clé 7");
    __map_set(m, padded, 99);
    assert_eq!(__map_get_ikey(m, 7), 8);
    __rc_release(padded);
    __map_set_ikey(m, 42, 1000);
    let keys = Map_keys(m, 0);
    assert_eq!(__array_len(keys), 8, "écriture d'une clé existante sans doublon");
    __rc_release(keys);
    __rc_release(m);
}

/// Petits blocs au-delà d'une liste de thread (réserve commune) et blocs
/// libérés par un autre thread : resservis, jamais deux fois à la fois.
#[test]
fn small_blocks_overflow_to_the_shared_pool_and_cross_threads() {
    use crate::alloc_cache::{alloc, dealloc};
    use std::alloc::Layout;
    use std::collections::HashSet;
    let layout = Layout::from_size_align(24, 8).unwrap();
    let blocks: Vec<usize> = (0..5000).map(|_| unsafe { alloc(layout) } as usize).collect();
    assert_eq!(blocks.iter().collect::<HashSet<_>>().len(), blocks.len(), "blocs distincts");
    let freed_elsewhere = blocks.clone();
    std::thread::spawn(move || {
        for b in freed_elsewhere {
            unsafe { dealloc(b as *mut u8, layout) };
        }
    }).join().unwrap();
    let again: Vec<usize> = (0..5000).map(|_| unsafe { alloc(layout) } as usize).collect();
    assert_eq!(again.iter().collect::<HashSet<_>>().len(), again.len(), "blocs distincts après réserve");
    for b in again {
        unsafe { dealloc(b as *mut u8, layout) };
    }
}

#[test]
fn integral_floats_keep_a_decimal_part() {
    assert_eq!(float_text(42.0), "42.0");
    assert_eq!(float_text(-3.0), "-3.0");
    assert_eq!(float_text(0.5), "0.5");
    assert_eq!(float_text(f64::INFINITY), "inf");
    assert_eq!(float_text(f64::NAN), "NaN");
}

/// `Array::join` : chaînes et `int` écrits directement, autres éléments par
/// le tampon — même texte dans tous les cas.
#[test]
fn join_writes_strings_ints_and_other_elements() {
    let joined = |arr: i64, sep: &str| unsafe {
        let sep = alloc_str(sep);
        let out = Array_join(arr, sep);
        let text = ptr_to_str(out).to_string();
        rc::release(out);
        rc::release(sep);
        text
    };
    let strings = new_array();
    for s in ["a", "é", "", "bc"] {
        array_push_owned(strings, unsafe { alloc_str(s) });
    }
    assert_eq!(joined(strings, "-"), "a-é--bc");
    assert_eq!(joined(strings, ""), "aébc");
    assert_eq!(joined(strings, " → "), "a → é →  → bc");
    let ints = new_array();
    rc::__rc_mark_raw_kind(ints, 1);
    for n in [0i64, -7, 1 << 40, i64::MIN] {
        __array_push(ints, n);
    }
    assert_eq!(joined(ints, ","), format!("0,-7,{},{}", 1i64 << 40, i64::MIN));
    let floats = new_array();
    rc::__rc_mark_raw_kind(floats, 2);
    __array_push(floats, 1.5f64.to_bits() as i64);
    __array_push(floats, 2.0f64.to_bits() as i64);
    assert_eq!(joined(floats, ";"), "1.5;2.0");
    let mixed = new_array();
    array_push_owned(mixed, unsafe { alloc_str("x") });
    array_push_owned(mixed, box_int_if_needed(70_000));
    array_push_owned(mixed, __box_bool(1));
    assert_eq!(joined(mixed, "|"), "x|70000|true");
    assert_eq!(joined(new_array(), "-"), "");
    for arr in [strings, ints, floats, mixed] {
        unsafe { rc::release(arr) }
    }
}

/// `s += `…`` (`__str_append_tpl`) : écrit au bout de `s` sans toucher une
/// copie partagée, gabarit qui relit `s`, parties à formater, grande
/// croissance (bloc agrandi sur place).
#[test]
fn template_append_writes_parts_after_the_string() {
    use crate::str_append::{__str_append, __str_append_tpl};
    let text = |v: i64| unsafe { ptr_to_str(v).to_string() };
    let append = |s: i64, parts: &[[i64; 2]]| __str_append_tpl(s, parts.as_ptr() as *const i64, parts.len() as i64);
    unsafe {
        let mut s = __str_append(alloc_str(""), alloc_str("ab"));
        let sep = alloc_str("-");
        s = append(s, &[[0, sep], [1, 42], [3, 1]]);
        assert_eq!(text(s), "ab-42true");

        rc::retain(s);
        let shared = s;
        s = append(s, &[[1, 7]]);
        assert_eq!(text(shared), "ab-42true", "la copie partagée ne change pas");
        assert_eq!(text(s), "ab-42true7");
        rc::release(shared);

        s = append(s, &[[0, s], [0, sep]]);
        assert_eq!(text(s), "ab-42true7ab-42true7-");

        s = append(s, &[[2, 1.5f64.to_bits() as i64]]);
        assert_eq!(text(s), "ab-42true7ab-42true7-1.5");

        let mut big = alloc_str("");
        for i in 0..100_000 {
            big = append(big, &[[1, i], [0, sep]]);
        }
        let expected: String = (0..100_000).map(|i| format!("{}-", i)).collect();
        assert_eq!(text(big), expected);
        rc::release(big);
        rc::release(s);
        rc::release(sep);
    }
}
