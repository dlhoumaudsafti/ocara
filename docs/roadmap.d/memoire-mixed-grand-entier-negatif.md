# Entier négatif de grande valeur dans un `mixed` : pas reconnu comme `int`

Statut : **corrigé** le 2026-10-13 (constaté le même jour par l'audit de la roadmap).

## Constat

```ocara
var n:mixed = 0 - 70000
n is int          // false ; un match sur n tombe dans default
var p:mixed = -5
p is int          // true
```

Déjà signalé en commentaire dans `examples/tests/37_mixed_large_intTest.oc`
(~l.55), sans suivi.

## Cause

`runtime/src/typecheck.rs` (~l.80-83) : le test « est un entier brut »
compare `val.abs()` à `PTR_THRESHOLD`, ce qui classe un grand négatif comme
pointeur possible. Un négatif n'est jamais une adresse valide.

## Correction attendue

Traiter toute valeur négative comme un entier dans `__is_int` et dans les
consommateurs de la même règle (affichage, comparaison, JSON).

## Validation

Assertions dans `examples/tests/37_mixed_large_intTest.oc` (`is int`, `match`,
affichage, `JSON::encode`) ; tests `runtime/src/tests/boxing.rs`.

## Priorité / Complexité

**Haute** (résultat faux) — **Simple**.

## Correctif

`__is_int` (`runtime/src/typecheck.rs`) traite toute valeur négative comme un entier brut (jamais une adresse). Les autres consommateurs (`get_value_type`) le faisaient déjà. Tests : `large_negative_raw_int_is_int` (`runtime/src/tests/boxing.rs`), `examples/tests/90_…` (`largeNegativeMixedIsIntTest`).
