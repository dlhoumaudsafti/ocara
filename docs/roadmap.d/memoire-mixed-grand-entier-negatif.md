# Entier négatif de grande valeur dans un `mixed` : pas reconnu comme `int`

Statut : **à corriger** — constaté le 2026-10-13 (audit de la roadmap).

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
