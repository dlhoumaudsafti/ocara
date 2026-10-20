# Dispatch, héritage, modules et génériques : valeurs fausses et code valide rejeté

**Clos le 2026-10-20.** Vérifié :
- Dispatch d'interface : les sous-classes (transitives) des implémenteurs ont leur branche (`implementers_with_descendants`, `src/lower/builder.d/interfaces.rs`).
- `parent::` dans une méthode ou un `init` hérité part du parent de la classe **déclarante** (`classes.rs`, `constructor_owner`) : `BA`, pas `BBA`.
- Monomorphisation en point fixe sur les corps spécialisés, mapping complet avant spécialisation, récursion polymorphe bornée (`src/core/monomorph.rs`).
- Méthodes de modules : une seule résolution (`classes::module_methods`, classe > dernier module > premier), utilisée par le lowering, le dispatcher de classe et les tables de signatures ; la sema compose les modules dans le même ordre (`registers.rs`). Plus de `__dispatch_C_greet` manquant ni de `DuplicateDefinition`.
- Un générique `implements I` est affectable à `I` (`types_compat`, `class_matches` sur les génériques).
- Soupçons : `self::` dans une méthode statique héritée est en liaison tardive, documenté (EBNF §16.5) ; `mixed(0) equal false` était un vrai bug, corrigé (`__cmp_eq_strict` distingue un bool boxé).
- Tests : `examples/tests/101_dispatch_inheritance_genericsTest.oc`, `src/core/tests_monomorph.rs`, `interfaces.rs` (`dispatcher_covers_subclasses_of_implementers`), `runtime/src/tests/boxing.rs`.


Audit du 2026-10-19. Reproduit sur le binaire courant.

## Valeurs fausses

1. **Appel via une interface sur une sous-classe de l'implémenteur → `null`.**
   `Base implements Named`, `Child extends Base`, `var a:Named = use Child()`,
   `a.name()` → `null` (attendu `base`), même si `Child` redéfinit `name()`.
   Le dispatcher ne liste que les classes qui écrivent `implements`
   (`src/lower/builder.d/interfaces.rs` ~l.35-37).
2. **`parent::` dans une méthode héritée part de la classe d'exécution.**
   `B.who()` rend `"B" + parent::who()`, `C extends B` sans redéfinition :
   `c.who()` → `BBA` au lieu de `BA` (`BBBA` avec un niveau de plus). Même
   chose pour un `init` hérité. `src/lower/builder.d/classes.rs` (~l.163,
   202) passe `class.extends` au lieu du parent de la classe déclarante.
3. **Générique instancié seulement dans le corps d'un générique : champs
   `null`.** `Pair<int,string>.swap(): Pair<B,A>` construit `use Pair<B, A>(…)`
   → `null`, `null` ; correct dès que `Pair<string,int>` est aussi instancié
   ailleurs. La monomorphisation ne fait pas de point fixe
   (`src/core/monomorph.rs`).

## Code valide rejeté

4. Module utilisé par une classe qui a une sous-classe :
   `codegen error: fonction interne non déclarée : __dispatch_C_greet`.
5. Deux modules qui définissent la même méthode : `DuplicateDefinition
   ("C_greet")` ; l'EBNF §19.4 dit que le dernier module l'emporte
   (`classes.rs` ~l.93-110).
6. Générique `implements I` non affectable à `I` : `expected type 'Sized',
   found 'Bag<string>'` (décrit par l'EBNF §20.4).

## Soupçons à confirmer

- `self::` dans une méthode statique héritée se comporte en liaison tardive
  (`B::viaSelf()` → `Bmake`) : voulu ? À documenter.
- `mixed(0) equal false` → `true`.

## Priorité / Complexité

**Haute** (valeurs fausses silencieuses) — **Légère** à **Structurel**
(point fixe de monomorphisation).

## Fichiers clés

`src/lower/builder.d/interfaces.rs`, `src/lower/builder.d/classes.rs`,
`src/core/monomorph.rs`, `src/sema/typecheck.rs`.
