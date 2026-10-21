# Performances : pistes d'optimisation restantes

Audit du 2026-10-19, mis à jour le 2026-10-21. Toutes les mesures de
`make perf` (`ci/perf/`) sont plus rapides que Python — tableau dans
`docs/benchmarking.md`. Ce qui reste, pour gagner encore :

1. **Variables locales en registres** : le code produit range chaque
   locale dans un emplacement de pile et la relit à chaque usage ; activer
   `opt_level = speed` de Cranelift ne gagne rien tant que les locales ne
   sont pas des `Variable` SSA du constructeur Cranelift (essayé le
   2026-10-21 : boucles et appels inchangés). Les boucles serrées restent 2 à
   4 fois plus lentes que `gcc -O2`.
2. **Méthode qui appelle du code** : `self` retenu puis relâché (~15 ns par
   appel, interface ~50 ns). Piste : prouver que l'appel ne peut pas libérer
   le receveur (receveur tenu par une locale non capturée de l'appelant).
3. **`String::split`** : marge faible sur Python (0,14 s contre 0,16 s) —
   un tableau coûte deux allocations (bloc `OcaraArray` et tampon du `Vec`).
4. **Mode multi-thread** : dès qu'un thread Ocara a été créé, comptes de
   références atomiques et verrou de chaque conteneur reviennent (≈ 10 ns
   par opération) — définitivement, même après la fin de ce thread.

## Priorité / Complexité

**Basse** (aucune mesure plus lente que Python) — **Structurel** (locales
SSA, retenue de `self`) à **Légère** (tableau en une allocation).

## Fichiers clés

`src/codegen/emit.d/`, `src/lower/builder.d/self_borrow.rs`,
`runtime/src/rc.rs`, `runtime/src/containers.rs`, `ci/perf_python.sh`.
