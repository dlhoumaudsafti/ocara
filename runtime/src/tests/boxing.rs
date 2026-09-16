// ─────────────────────────────────────────────────────────────────────────────
// Tests systématiques — durcissement de la représentation `mixed`
// (docs/roadmap.d/memoire-boxing-durcissement.md, volet 1).
//
// Contrairement aux tests de régression `.oc` existants (un cas précis par bug
// déjà trouvé par reproduction — voir docs/roadmap.d/memoire-fiabilite-runtime-
// bas-niveau.md), ce module balaie systématiquement l'espace des valeurs
// frontières autour du seuil de boxing (`PTR_THRESHOLD`) plutôt qu'un point
// unique par bug historique — objectif : détecter une future variante de la
// même classe de bug directement ici, avant une reproduction par SEGFAULT sur
// un programme `.oc` complet.
// ─────────────────────────────────────────────────────────────────────────────

use crate::*;

// ── Groupe 1 : round-trip de boxing autour de PTR_THRESHOLD (int/float/bool) ──

#[test]
fn box_int_if_needed_small_nonzero_stays_raw() {
    for n in [1i64, 2, 100, 65535] {
        let boxed = box_int_if_needed(n);
        assert_eq!(boxed, n, "un petit entier non nul doit rester brut (non alloué)");
        assert!(!is_int_box(boxed));
    }
}

#[test]
fn box_int_if_needed_zero_is_always_boxed() {
    // Coeur du correctif de langage-array-get-display-bug.md : `0` doit
    // être distinguable de `null` (qui est aussi le bit pattern 0).
    let boxed = box_int_if_needed(0);
    assert_ne!(boxed, 0, "0 boxé ne doit jamais être le pointeur nul");
    assert!(is_int_box(boxed));
    assert_eq!(unsafe { unbox_int(boxed) }, 0);
    assert_eq!(get_value_type(0), 0, "un 0 NON boxé reste indiscernable de null (c'est justement pourquoi il doit être boxé)");
    assert_eq!(get_value_type(boxed), 1, "un 0 boxé doit être vu comme primitif, jamais comme null");
}

#[test]
fn box_int_if_needed_boundary_around_ptr_threshold() {
    // 65536 = PTR_THRESHOLD, redéfini séparément à trois endroits du
    // runtime (littéral 0x10000 dans is_ptr/is_float_box/is_bool_box/
    // is_int_box, const PTR_THRESHOLD privée ici, const PTR_THRESHOLD
    // privée dans typecheck.rs) — ce test fige le comportement observable
    // à la frontière plutôt que de comparer les constantes entre elles
    // (certaines ne sont pas accessibles hors de leur module).
    let below = box_int_if_needed(65535);
    assert_eq!(below, 65535, "65535 doit rester brut");
    assert!(!is_int_box(below));

    let at = box_int_if_needed(65536);
    assert!(is_int_box(at), "65536 doit être boxé (ambigu avec un pointeur heap)");
    assert_eq!(unsafe { unbox_int(at) }, 65536);

    let above = box_int_if_needed(65537);
    assert!(is_int_box(above));
    assert_eq!(unsafe { unbox_int(above) }, 65537);
}

#[test]
fn box_int_if_needed_negative_always_raw() {
    // Un entier négatif n'est jamais ambigu avec un pointeur heap (toujours
    // < PTR_THRESHOLD en comparaison signée) — voir la doc de la fonction.
    for n in [-1i64, -65536, -1_000_000, i64::MIN] {
        let boxed = box_int_if_needed(n);
        assert_eq!(boxed, n, "un entier négatif doit toujours rester brut, quelle que soit sa magnitude : {n}");
    }
}

#[test]
fn box_int_if_needed_large_positive_boxed_and_roundtrips() {
    for n in [1_000_000i64, i64::MAX / 2, i64::MAX] {
        let boxed = box_int_if_needed(n);
        assert!(is_int_box(boxed), "doit être boxé : {n}");
        assert_eq!(unsafe { unbox_int(boxed) }, n);
        assert_eq!(get_value_type(boxed), 1);
    }
}

#[test]
fn float_boxing_roundtrip_boundary_values() {
    for f in [0.0f64, -0.0, 1.5, -1.5, f64::MIN, f64::MAX, f64::EPSILON, f64::MIN_POSITIVE] {
        let boxed = __box_float(f.to_bits() as i64);
        assert!(is_float_box(boxed), "doit porter le tag float : {f}");
        assert!(!is_int_box(boxed) && !is_bool_box(boxed));
        assert_eq!(__unbox_float(boxed), f);
        assert_eq!(get_value_type(boxed), 1, "un float boxé est un primitif, jamais confondu avec un objet tas");
    }
}

#[test]
fn float_boxing_nan_roundtrips_by_bit_pattern() {
    let nan = f64::NAN;
    let boxed = __box_float(nan.to_bits() as i64);
    assert!(is_float_box(boxed));
    assert!(__unbox_float(boxed).is_nan());
}

#[test]
fn bool_boxing_roundtrip() {
    for b in [0i64, 1] {
        let boxed = __box_bool(b);
        assert!(is_bool_box(boxed));
        assert!(!is_int_box(boxed) && !is_float_box(boxed));
        assert_eq!(__unbox_bool(boxed), b);
        assert_eq!(get_value_type(boxed), 1);
    }
}

#[test]
fn null_is_never_confused_with_a_boxed_value() {
    assert_eq!(get_value_type(0), 0);
    assert!(!is_int_box(0) && !is_float_box(0) && !is_bool_box(0) && !is_ptr(0));
}

// ── Groupe 2 : conteneurs imbriqués — profondeur 5, free/clone `_concrete` ──

/// Construit un `array<array<array<array<array<int>>>>>` (profondeur 5) où
/// la feuille est un entier BRUT choisi pour "ressembler" à un pointeur
/// heap valide (aligné sur 8, >= PTR_THRESHOLD) — si le chemin `_concrete`
/// régressait vers le chemin générique (`__value_free`/`__value_clone` par
/// élément), une telle feuille serait déréférencée par `read_tag` et
/// planterait le process (même famille que le SEGFAULT historique sur
/// `array<float>`, voir docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md).
fn build_nested_int_array(depth: usize, pointer_shaped_leaf: i64) -> i64 {
    let mut current = new_array();
    unsafe { array_ref(current).data.push(pointer_shaped_leaf); }
    for _ in 1..depth {
        let outer = new_array();
        unsafe { array_ref(outer).data.push(current); }
        current = outer;
    }
    current
}

#[test]
fn free_concrete_depth_5_does_not_dereference_pointer_shaped_leaves() {
    let leaf = 1_048_576i64; // 0x100000 : aligné sur 8, >= PTR_THRESHOLD
    let arr = build_nested_int_array(5, leaf);
    let shape = unsafe { alloc_str("AAAA") }; // 4 niveaux sous le top-level, profondeur totale 5
    __array_free_concrete(arr, shape, 0);
    unsafe { free_str(shape); }
    // Ne pas planter ici EST le test.
}

#[test]
fn clone_concrete_depth_5_preserves_values_and_is_independent() {
    let leaf = 2_097_152i64; // 0x200000 : aligné, également pointer-shaped
    let arr = build_nested_int_array(5, leaf);
    let shape = unsafe { alloc_str("AAAA") };

    let cloned = __array_clone_concrete(arr, shape, 0);
    assert_ne!(cloned, arr, "le clone doit être un array indépendant, pas le même pointeur");

    // Redescend les 4 niveaux intermédiaires du clone pour vérifier que la
    // feuille a survécu intacte à la copie profonde.
    let mut level = cloned;
    for _ in 0..4 {
        level = unsafe { array_ref(level).data[0] };
    }
    assert_eq!(unsafe { array_ref(level).data[0] }, leaf);

    __array_free_concrete(arr, shape, 0);
    __array_free_concrete(cloned, shape, 0);
    unsafe { free_str(shape); }
}

#[test]
fn free_concrete_mixed_array_of_maps_of_arrays() {
    // array<map<string, array<int>>> — exerce le branchement 'A'/'M' de
    // concrete_elem_shape dans les deux sens, pas seulement des arrays imbriqués.
    let inner_arr = new_array();
    unsafe { array_ref(inner_arr).data.push(999_999); }
    let inner_map = new_map();
    unsafe { map_ref(inner_map).data.push(("k".to_string(), inner_arr)); }
    let top = new_array();
    unsafe { array_ref(top).data.push(inner_map); }

    let shape = unsafe { alloc_str("MA") }; // niveau 1 = map, niveau 2 = array
    __array_free_concrete(top, shape, 0);
    unsafe { free_str(shape); }
}

// ── Groupe 3 : comparaisons strictes — 6 comparateurs × 4 tags ──

#[test]
fn eq_strict_raw_int_vs_boxed_int_same_value() {
    let raw = 42i64;
    let boxed = box_int_if_needed(1_000_000);
    let boxed_same = box_int_if_needed(1_000_000);
    assert_eq!(__cmp_eq_strict(raw, raw), 1);
    assert_eq!(__cmp_eq_strict(boxed, boxed_same), 1, "deux boxings indépendants de la même valeur doivent comparer égaux (par valeur, pas par adresse)");
    assert_eq!(__cmp_ne_strict(boxed, boxed_same), 0);
}

#[test]
fn eq_strict_float_boxed_vs_raw_int_coerces_numerically() {
    // cmp_primitive : dès qu'UN opérande est float-boxé, les DEUX sont
    // comparés en flottant — un entier brut est converti, jamais comparé
    // par bit pattern.
    let f = __box_float(3.0f64.to_bits() as i64);
    assert_eq!(__cmp_eq_strict(f, 3), 1);
    assert_eq!(__cmp_lt_strict(f, 4), 1);
    assert_eq!(__cmp_gt_strict(f, 2), 1);
}

#[test]
fn ordering_comparators_on_boxed_ints() {
    let a = box_int_if_needed(100_000);
    let b = box_int_if_needed(200_000);
    assert_eq!(__cmp_lt_strict(a, b), 1);
    assert_eq!(__cmp_gt_strict(b, a), 1);
    assert_eq!(__cmp_le_strict(a, a), 1);
    assert_eq!(__cmp_ge_strict(a, a), 1);
    assert_eq!(__cmp_lt_strict(b, a), 0);
}

#[test]
fn eq_strict_type_mismatch_is_always_false() {
    let s = unsafe { alloc_str("42") };
    let i = box_int_if_needed(1_000_000);
    assert_eq!(__cmp_eq_strict(s, i), 0, "types différents (string vs primitif) -> toujours 0, quel que soit le contenu");
    assert_eq!(__cmp_ne_strict(s, i), 1);
    assert_eq!(__cmp_lt_strict(s, i), 0);
    unsafe { free_str(s); }
}

#[test]
fn string_eq_strict_compares_content_not_pointer() {
    let a = unsafe { alloc_str("bonjour") };
    let b = unsafe { alloc_str("bonjour") };
    assert_ne!(a, b, "deux allocations distinctes doivent avoir des adresses différentes");
    assert_eq!(__cmp_eq_strict(a, b), 1, "mais comparer égales par contenu");
    unsafe { free_str(a); free_str(b); }
}

#[test]
fn heap_object_eq_strict_is_pointer_identity_not_structural() {
    // Deux arrays de contenu identique mais d'adresses différentes NE
    // comparent PAS égaux (seul le type primitif/string a une comparaison
    // par valeur, voir __cmp_eq_strict) — comportement figé ici pour que
    // toute évolution future de ce choix soit délibérée, pas accidentelle.
    let a = new_array();
    let b = new_array();
    unsafe {
        array_ref(a).data.push(1);
        array_ref(b).data.push(1);
    }
    assert_eq!(__cmp_eq_strict(a, b), 0);
    assert_eq!(__cmp_eq_strict(a, a), 1);
    __array_free(a);
    __array_free(b);
}

#[test]
fn bool_boxed_and_int_compare_by_numeric_value() {
    // Comportement actuel figé ici (pas nécessairement souhaitable, voir
    // la limitation documentée pour `is bool` dans docs/EBNF.md §6.3) : un
    // bool boxé `true` et un entier brut `1` comparent égaux, car
    // cmp_primitive traite les deux comme des i64 numériques dès qu'aucun
    // côté n'est float-boxé.
    let t = __box_bool(1);
    assert_eq!(__cmp_eq_strict(t, 1), 1);
}

// ── Groupe 4 : strings avec NUL interne, à toute position ──

#[test]
fn nul_byte_string_roundtrip_start_middle_end() {
    for s in ["\0abc", "ab\0c", "abc\0", "\0\0double", "a\0\0\0b"] {
        let ptr = unsafe { alloc_str(s) };
        let back = unsafe { ptr_to_str(ptr) };
        assert_eq!(back, s, "la longueur réelle doit être préservée, pas tronquée au premier NUL");
        assert_eq!(back.len(), s.len());
        assert_eq!(__is_string(ptr), 1);
        unsafe { free_str(ptr); }
    }
}

#[test]
fn nul_byte_strings_compare_by_full_content_not_truncated_prefix() {
    let a = unsafe { alloc_str("a\0b") };
    let b = unsafe { alloc_str("a\0c") };
    // Si la comparaison tronquait au premier NUL, "a\0b" et "a\0c"
    // deviendraient toutes deux "a" et compareraient (à tort) égales.
    assert_eq!(__cmp_eq_strict(a, b), 0);
    assert_eq!(__cmp_ne_strict(a, b), 1);
    unsafe { free_str(a); free_str(b); }
}

#[test]
fn nul_only_string_is_not_empty() {
    let ptr = unsafe { alloc_str("\0") };
    let back = unsafe { ptr_to_str(ptr) };
    assert_eq!(back.len(), 1, "une string composée d'un seul NUL a une longueur 1, pas 0");
    unsafe { free_str(ptr); }
}

// ── Groupe 5 : __value_free/__value_clone sur une cellule boxée ──
// (docs/roadmap.d/memoire-boxing-durcissement.md, « Découverte en
// écrivant le volet 1 » — avant ce correctif, __value_free ne libérait
// jamais une cellule boxée, et __value_clone en retournait un ALIAS,
// pas une copie : combiner les deux aurait donné un double-free/UAF.)

#[test]
fn value_free_frees_a_boxed_primitive_without_crashing() {
    // Ne pas planter ici EST le test : avant ce correctif, __value_free
    // ne reconnaissait aucune des trois cellules boxées (elles fuyaient
    // silencieusement, pas de crash observable — ce test garantit
    // seulement que le nouveau chemin de libération est correct).
    __value_free(box_int_if_needed(1_000_000));
    __value_free(__box_float(3.5f64.to_bits() as i64));
    __value_free(__box_bool(1));
}

#[test]
fn value_clone_of_boxed_primitive_is_an_independent_cell() {
    let original = box_int_if_needed(424_242);
    let cloned = __value_clone(original);

    assert_ne!(original, cloned, "un clone doit être une allocation distincte, pas un alias du même pointeur");
    assert_eq!(unsafe { unbox_int(cloned) }, 424_242, "la valeur doit survivre à la copie profonde");

    // Propriété de sécurité critique : libérer l'ORIGINAL ne doit rien
    // faire au CLONE. Avant ce correctif, __value_clone retournait le
    // même pointeur (alias) : ce test aurait alors provoqué un
    // use-after-free/double-free sur `cloned` juste après.
    __value_free(original);
    assert_eq!(unsafe { unbox_int(cloned) }, 424_242, "le clone doit rester lisible après libération de l'original");
    __value_free(cloned);
}

#[test]
fn array_clone_then_free_original_leaves_boxed_element_in_clone_intact() {
    // Reproduction fidèle du scénario documenté en docs/EBNF.md §9.2 :
    // une `scoped array<mixed>` qui s'échappe (`var y = x`) reçoit une
    // "copie profonde indépendante" ; la détruire ensuite (fin de bloc)
    // ne doit jamais affecter la copie. Avant ce correctif, un élément
    // boxé de l'array n'était pas réellement dupliqué par __array_clone
    // (qui délègue à __value_clone par élément) : les deux arrays
    // auraient partagé la même cellule boxée.
    let original = new_array();
    unsafe { array_ref(original).data.push(box_int_if_needed(7_000_000)); }

    let cloned = __array_clone(original);
    assert_ne!(cloned, original);

    __array_free(original);

    let elem = unsafe { array_ref(cloned).data[0] };
    assert_eq!(unsafe { unbox_int(elem) }, 7_000_000, "l'élément boxé du clone doit rester valide après libération de l'array original");

    __array_free(cloned);
}
