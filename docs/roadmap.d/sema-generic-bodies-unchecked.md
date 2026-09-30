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

## Piste

Typechecker chaque `generic` avec ses paramètres de type liés à des types
abstraits (ou vérifier les classes monomorphisées, en déplaçant la
monomorphisation avant la sema) — à trancher, les deux ont des implications
sur les messages d'erreur (position dans le `generic` vs dans
l'instanciation).

## Priorité / Complexité

**Moyenne** — **Structurel**.

## Fichiers clés

`src/sema/typecheck.rs` (`check_program`), `src/core/monomorph.rs`,
`src/main.rs` (ordre sema/monomorphisation), `src/core/named_args.rs`
(repli syntaxique à retirer une fois ce ticket traité).
