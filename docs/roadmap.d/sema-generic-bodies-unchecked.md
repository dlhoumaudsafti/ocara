# Corps des `generic` jamais parcourus par l'analyse sémantique

## Constat (vérifié dans le code)

`TypeChecker::check_program` (`src/sema/typecheck.rs`) vérifie les blocs
runtime, les fonctions libres et `program.classes` — jamais
`program.generics`. La monomorphisation (`core::monomorph`) tourne APRÈS la
sema : les classes concrètes qu'elle produit (`List<int>` → classe
spécialisée) ne sont pas non plus vérifiées. Conséquence : aucune erreur de
type, d'arité, de symbole indéfini, d'échappement de ressource, etc. n'est
détectée dans une méthode de `generic`.

Mis en évidence par les arguments nommés : un appel nommé dans un corps de
`generic` n'est pas résolu par la sema. Contourné pour ce ticket par une
résolution syntaxique dans `core::named_args` (fonction libre,
`Classe::m(...)`, `self::m(...)`, `self.m(...)`, `use X(...)`) ; un appel
nommé sur un autre receveur y reste une erreur explicite (voir E50 dans
`docs/diagnostics.md`).

## Décision et correction

**Retenu (David)** : vérification UNE fois par generic, `T` permissif (et
non par instanciation monomorphisée — un generic jamais instancié serait
resté non vérifié, et une erreur répétée par instanciation).

- `core::monomorph::erased_generic_class` : classe « effacée » du generic,
  paramètres de type remplacés par `mixed` — construite par le même code de
  substitution que la monomorphisation (`specialize_members`, factorisé) ;
- `sema::generic_check::check_generic` : vérifie cette classe ;
  `current_generic` suspend les diagnostics propres à `mixed` (E14/E15/
  W02/W03 — un `T` n'est pas un `mixed` écrit par le développeur) et type
  `self` comme une instance du generic (`self.m(...)` résolu sur ses
  méthodes, arguments nommés compris) ;
- **même défaut découvert sur les `module` (mixins)**, jamais vérifiés non
  plus — `check_module` les vérifie une fois comme une classe (accès
  `self.x` à la classe utilisatrice permissifs) ;
- tout le code passant désormais par la sema, le repli syntaxique des
  arguments nommés de `core::named_args` est supprimé.

Tests : `src/sema/tests/generic_check.rs`. Documenté dans `docs/EBNF.md`
§20.6. Aucun exemple existant ne contenait d'erreur ainsi révélée.

## Priorité / Complexité

**Moyenne** — **Structurel**.

## Fichiers clés

`src/sema/typecheck.rs` (`check_program`), `src/core/monomorph.rs`,
`src/main.rs` (ordre sema/monomorphisation), `src/core/named_args.rs`
(repli syntaxique à retirer une fois ce ticket traité).
