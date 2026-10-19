# Scalaire (`int`/`float`) placé dans une union sans être boxé — corrigé

Audit du 2026-10-19, corrigé le même jour.

## Constat

Un `int` ou un `float` qui entrait dans un type union (`int|string`,
`int|float`, `float|null`…) par un `return`, un `emit` ou l'écriture d'une
propriété n'était pas boxé : la valeur brute était retenue comme un pointeur
(`__rc_retain`), d'où un SIGSEGV dès 65 536 (`PTR_THRESHOLD`) ou pour tout
float.

```ocara
function box(n:int): int|string { return n }          // box(70000) → SIGSEGV
function k(): int|float { return 2.5 }                  // SIGSEGV
class C { public property v:int|float = 1.25 }          // affichage → SIGSEGV
```

Même famille :

- valeur d'union affinée par `is` puis renvoyée en scalaire
  (`if v is float { return v }`, `return v / 2.0`) : non déballée, un
  dénormal au lieu de `3` ;
- gabarit `${array<float>}` : SIGSEGV (`${array<int>}` avec 100 000 et
  `${array<bool>}` faux).

## Correctif

- `returned_value` (`src/lower/stmt.d/statements.rs`) : `box_for_any` vers
  `Ptr` pour un retour `mixed` **ou union**, et déballage vers le scalaire
  pour un retour `int`/`float`/`bool` — la même conversion que
  `var r:T = expr`.
- Propriété `mixed` **ou union** : boxée à l'affectation
  (`src/lower/stmt.d/statements.d/assignments.rs`), initialiseurs de
  `property` compris.
- `emit` dans un `message<mixed>` ou `message<union>` : boxé dans
  `store_emitted` (`src/lower/builder.d/message_gen.rs`), sans retenue en
  double d'une cellule neuve.
- `__array_to_str` formate chaque élément selon le genre brut du tableau
  (`fmt_elem`, `runtime/src/lib.rs`).

## Tests

`examples/tests/95_union_scalar_boxingTest.oc` (retours de fonction, méthode,
statique et `nameless`, `emit`, propriétés, valeurs affinées, gabarits de
tableaux bruts) ; `--leak-check` sans fuite.
