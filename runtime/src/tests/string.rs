// ─────────────────────────────────────────────────────────────────────────────
// Tests unitaires — builtins `String::*` (voir docs/builtins/String.md).
// ─────────────────────────────────────────────────────────────────────────────

use crate::*;

fn replace(s: &str, from: &str, to: &str) -> String {
    unsafe {
        let r = String_replace(alloc_str(s), alloc_str(from), alloc_str(to));
        ptr_to_str(r).to_string()
    }
}

/// docs/roadmap.d/stdlib-string-replace-first-only.md — remplaçait
/// seulement la première occurrence (`replacen(..., 1)`).
#[test]
fn replace_replaces_every_occurrence() {
    assert_eq!(replace("a_b_c", "_", "-"), "a-b-c");
    assert_eq!(replace("chat noir chat blanc", "chat", "chien"), "chien noir chien blanc");
}

#[test]
fn replace_without_occurrence_returns_source() {
    assert_eq!(replace("abc", "x", "-"), "abc");
}

#[test]
fn replace_with_empty_pattern_returns_source_unchanged() {
    assert_eq!(replace("abc", "", "-"), "abc");
}

#[test]
fn replace_can_remove_occurrences() {
    assert_eq!(replace("a-b-c", "-", ""), "abc");
}
