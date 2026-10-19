# Scalaire (`int`/`float`) placé dans une union sans être boxé : SIGSEGV ou valeur fausse

Audit du 2026-10-19. Reproduit sur le binaire courant.

## Constat

Un `int` ou un `float` qui entre dans un type union (`int|string`, `int|float`,
`float|null`…) par un `return` ou par l'écriture d'une propriété n'est pas
boxé. La valeur brute est ensuite traitée comme un pointeur (`__rc_retain`,
affichage) :

- `int` ≥ 65 536 (`PTR_THRESHOLD`, `runtime/src/typecheck.rs:56`) ou `float`
  (ses bits ressemblent à une adresse) : **SIGSEGV** dans `__rc_retain`.
- `int` plus petit : valeur affichée fausse ou `null`.

```ocara
function box(n:int): int|string { return n }          // SIGSEGV avec box(70000)
function k(): int|float { return 2.5 }                  // var x:int|float = k() → SIGSEGV
class C { public property v:int|float = 1.25 }          // affichage de c.v → SIGSEGV
function k2(x:float): float|null { return x }           // y + 1.0 après `is null` → SIGSEGV
```

Cas qui fonctionnent : la variable locale (`var r:int|string = n; return r`),
`m["a"] = n` sur une `map<string, int|string>`, l'argument `show(80000)`,
`array<int|string>`.

Même famille :

- **Float affiné renvoyé depuis une union** : `function e(v:int|float): float
  { if v is float { return v } return 0.5 }` → `e(3.0)` affiche un dénormal
  (`0.000…0195`) au lieu de `3`.
- **Gabarit `${…}` sur un `array<float>`** : SIGSEGV.

## Piste

Le chemin `return` et l'affectation de propriété vers un type union doivent
appliquer la même conversion que la déclaration locale (`box_for_dyn_arith` /
`__box_int_for_mixed`, `__box_float`). Vérifier aussi les paramètres par
défaut et les champs de génériques.

## Priorité / Complexité

**Haute** (crash) — **Légère**.

## Fichiers clés

`src/lower/stmt.d/statements.d/` (return), `src/lower/stmt.d/statements.d/assign*`
(propriétés), `src/lower/expr.d/helpers.rs` (`box_for_dyn_arith`).
