# `Array::join/sort/contains/indexOf`, affichage des conteneurs et `UnitTest` : éléments bruts mal lus

Audit du 2026-10-19. Reproduit sur le binaire courant.

## Constat

Plusieurs fonctions du runtime lisent les éléments d'un tableau sans tenir
compte de leur genre brut (`FLAG_RAW`, entiers et flottants non boxés) ni du
contenu des chaînes :

1. **`Array::join` / `Convert::arrayToStr` / `arr.join()` / `arr.toStr()` →
   SIGSEGV** sur `array<int>` (valeur ≥ 0x10000 dont les 2 bits bas valent 0,
   1 ou 2 : 65 537, 70 000, 100 000…) et sur `array<float>`. Pour `[0, 1, 2]`
   → `null,1,2` ; `[true, false]` → `1,null`. `Array_join`
   (`runtime/src/lib.rs` ~l.1137) formate avec `fmt_val` sans le genre du
   tableau.
2. **`sort`, `contains`, `indexOf` comparent les chaînes par adresse.**
   `[`z${x}`, `a${x}`, `m${x}`]` → `sort` rend `zb,ab,mb` ; `contains(w,
   `a${x}`)` → `false`. Fonctionne aujourd'hui seulement grâce à l'internement
   des littéraux identiques (`lib.rs` ~l.1105, 1111, 1146).
3. **`sort` sur `array<float>` trie les motifs de bits** :
   `[3.5, -1.0, 2.25, -10.5]` → `-1, -10.5, 2.25, 3.5`.
4. **`IO::writeln` et gabarits sur conteneurs** : `IO::writeln(array<int>)` et
   `IO::writeln(map)` impriment des octets bruts ; `${map}` → `[1, ]` ;
   `${array<array<int>>}` → octets bruts (le gabarit d'un tableau à éléments
   bruts `int`/`float`/`bool` est corrigé).
5. **Assertions `UnitTest`** : `assertGreater(2.5, 1.5)` affiche des adresses
   de boîtes ; `assertEmpty([])` échoue et `assertNotEmpty([])` passe ;
   `assertEquals([1,2],[1,2])` échoue (`expected    but got   `) ;
   `assertEnum` passe toujours (doc : code 117). `ptr_to_str` appliqué aux
   tableaux (`lib.rs` ~l.1880-2052).

## Piste

Le gabarit `${array<…>}` est corrigé (2026-10-19) : `__array_to_str` passe
par `fmt_elem(raw_kind, v)` (`runtime/src/lib.rs`), à réutiliser pour
`Array_join`, l'affichage `IO::writeln` et `UnitTest`.

Un seul formateur d'élément qui reçoit le genre du conteneur (comme
`value_to_json` avec `kind | depth << 8`) et une comparaison par contenu pour
les chaînes, partagés par join/sort/contains/indexOf/affichage/UnitTest.

## Priorité / Complexité

**Haute** (crash, valeurs fausses) — **Légère**.

## Fichiers clés

`runtime/src/lib.rs` (`Array_*`, `fmt_val`, `ut_val_to_display`),
`runtime/src/typecheck.rs`, `src/lower/expr.d/helpers.rs` (`static_leaf_shape`).
