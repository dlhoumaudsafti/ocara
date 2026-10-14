# `UnitTest::assertEnum` échoue toujours sur une variante d'enum

Statut : **à corriger** — constaté le 2026-10-13 (documentation de `docs/builtins/UnitTest.md`).

## Constat

```ocara
enum Color { RED, GREEN }
UnitTest::assertEnum(Color::RED)   // UnitTestException : « assertEnum: value is not an enum » (code 117)
```

`UnitTest_assertEnum` (`runtime/src/lib.rs`) suppose que les enums sont des objets (`__is_object`), alors qu'une variante d'enum est compilée en entier : l'assertion ne peut jamais réussir sur une vraie variante, et passerait sur n'importe quel objet.

## Correction attendue (à trancher)

- vérification à la compilation : la sema sait si l'argument est de type enum ; le lowering remplace l'appel par un succès (`ut_pass`) ou une erreur de compilation ;
- ou retrait de `assertEnum` (une variante est un `int`, `assertEquals` suffit), avec dépréciation documentée.

## Fichiers concernés

`runtime/src/lib.rs` (`UnitTest_assertEnum`), `src/builtins/unittest.rs`, `src/sema/typecheck.rs`, `docs/builtins/UnitTest.md`.

## Validation

Test `examples/tests/` : `assertEnum(Color::RED)` passe ; un entier ou un objet quelconque est refusé (selon l'option retenue).

## Priorité / Complexité

**Haute** (assertion builtin toujours fausse) — **Simple**.

## Correctif

Choix retenu : **vérification à la compilation**. La sema exige pour `UnitTest::assertEnum(x)` une variante d'enum (`Color::RED`) ou une valeur de type enum, sinon « expected type 'enum' » ; le runtime valide alors l'assertion. Tests : `src/sema/tests/structural_fixes.rs`, `examples/tests/91_…` (`assertEnumTest`).
