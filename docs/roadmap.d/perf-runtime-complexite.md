# Performances : pistes d'optimisation restantes

Audit du 2026-10-19, mis à jour le 2026-10-21. Les 33 mesures de `make perf`
et celles de `make perf-extra` (démarrage, mémoire, serveur HTTP) sont
meilleures que Python — tableau dans `docs/benchmarking.md`. Ce qui reste,
pour gagner encore :

1. **Locales encore en mémoire** : une locale dont l'adresse ne sert qu'à
   ses lectures et écritures est une `Variable` SSA de Cranelift (registre).
   Restent en pile : les locales d'une fonction qui appelle `setjmp`
   (générateurs — un registre ne survit pas à `longjmp`) et celles dont
   l'adresse est transmise (cellules de capture des `nameless` et des
   `try`). `opt_level = speed` essayé le 2026-10-21 après le passage en
   registres : aucun gain (boucle `loop` 0,06 → 0,09 s), réglage par défaut
   conservé.
2. **`self` retenu quand l'appelée peut le libérer** : un appel
   `self.m(…)` dont toutes les définitions de `m` sont sûres ne retient plus
   `self` (`meth` 0,15 → 0,01 s). Une méthode qui appelle autre chose (une
   fonction, une méthode d'un autre objet, un builtin) retient et relâche
   toujours `self` (≈ 15 ns). Piste : prouver côté appelant que le receveur
   reste tenu (locale non capturée ni réaffectée pendant l'appel).

## Écarté

- **Retour au mode mono-thread après la fin des threads** : les comptes de
  références redeviennent atomiques dès qu'un thread Ocara a existé
  (≈ 10 ns par opération, `mt` 0,09 s contre 0,53 s pour Python). Revenir
  en arrière n'est pas sûr : des threads du runtime (ouvriers du serveur
  HTTP, pool des tâches `async`, rappels Tauri) entrent dans du code Ocara
  sans passer par le thread qui crée ; un passage en mode non atomique
  pendant une entrée corromprait des comptes.

## Priorité / Complexité

**Basse** (aucune mesure plus lente que Python) — **Structurel** (preuve
côté appelant, locales capturées hors cellule).

## Fichiers clés

`src/codegen/emit.d/value_slots.rs`, `src/lower/builder.d/self_borrow.rs`,
`src/lower/expr.d/lower.rs` (promotion des captures), `ci/perf_python.sh`.
