# Littéral `array`/`map` dont l'élément est une union de primitifs — SIGSEGV

Statut : **corrigé** le 2026-10-13 (constaté le même jour par l'audit de la roadmap).

## Constat

Un programme valide plante dès que le littéral contient un flottant, un grand
entier ou `null` :

```ocara
var a1:array<int|float> = [1, 2.5]           // SIGSEGV
var a2:array<float|null> = [2.5, null]       // SIGSEGV
var a3:array<int|float> = [70000, 3]         // SIGSEGV
var m:map<string, int|float> = {"a": 2.5}    // SIGSEGV
take([1.5])                                  // take(xs:array<int|float>) : SIGSEGV
```

`array<int|float> = [1, 2]` passe (petits entiers), `push(2.5)` passe aussi
mais fuit (voir [memoire-boxing-temporaires-non-relaches](memoire-boxing-temporaires-non-relaches.md)).

## Cause

`lower_literal_element` (`src/lower/expr.d/lower.rs`, ~l.210) ne boxe un
élément que si le type d'élément déclaré est exactement `Type::Mixed`. Pour une
union, la valeur brute (bits d'un `float`, grand entier) est rangée dans un
conteneur compté qui la suit ensuite comme un pointeur. Probable régression du
correctif de [memoire-nested-array-zero-json](memoire-nested-array-zero-json.md).

## Correction attendue

Boxer tout élément dont le type déclaré est une union (ou `mixed`), comme le
fait déjà `push`/l'affectation indexée ; vérifier les littéraux imbriqués et
les arguments.

## Validation

Test `examples/tests/` couvrant les cinq cas ci-dessus (lecture des valeurs,
`--leak-check` sans fuite) ; `make regression`.

## Priorité / Complexité

**Haute** (crash d'un programme valide, usage courant) — **Légère**.

## Correctif

`lower_literal_element` (`src/lower/expr.d/lower.rs`) boxe aussi l'élément quand le type déclaré est une union, comme pour `mixed`. Test : `examples/tests/88_containers_receivers_and_foreachTest.oc` (`unionLiteralTest`).
