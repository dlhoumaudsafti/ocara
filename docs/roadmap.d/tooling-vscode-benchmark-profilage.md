# Extension VS Code : nouvelles fonctions de benchmark et de profilage

Proposé le 2026-10-19 à la suite de l'audit (techniques décrites dans
`docs/benchmarking.md`). Fonctionnalités nouvelles : aucune n'est un défaut.

## Existant

Encart « Benchmark » (`tools/highlight/vsode/src/benchmark.ts`) : temps et
mémoire (`/usr/bin/time`), fuites (`--leak-check`, TSV `OCARA_LEAK_OUT`),
croissance à deux tailles (`__N__`), profil par fonction (`--profile`, TSV
`OCARA_PROFILE_OUT`), historique de 30 mesures par script.

## Propositions

1. **Courbe de complexité (`__N__`)** — 4 ou 5 tailles
   (`ocara.benchmark.growthSizes`), 3 exécutions par taille (min), pente
   log-log `log(t2/t1)/log(N2/N1)` : verdict O(n), O(n log n) ou O(n²) ; une
   pente > 1,5 donne « super-linéaire ». Graphique en webview.
2. **Statistiques robustes** — min et médiane plutôt que la moyenne, écart
   relatif, avertissement « machine chargée » quand `os.loadavg()[0] >
   os.cpus().length * 0.5`, charge stockée dans l'historique.
3. **Comparaison A/B** — deux scripts ou deux compilateurs (`compilerPath` A
   et B), exécutions alternées ABAB, rapport de vitesse.
4. **Compteurs du runtime dans le profil** — retain/release, collectes de
   cycles et leur durée, candidats racines, exceptions levées, threads et
   tâches créés. CLI : lignes `R<TAB>compteur<TAB>valeur<TAB>ns` ajoutées à
   `OCARA_PROFILE_OUT` sous `--profile` (compteurs atomiques dans
   `runtime/src/profile.rs`) ; groupe « Runtime » dans l'encart.
5. **Profil cliquable et CodeLens** — fichier et ligne ajoutés au TSV du
   profil (table `src/core/leak_sites.rs`), CodeLens « propre X ms · N
   appels » au-dessus de chaque fonction profilée ; lieux de fuite affichés
   comme diagnostics d'information.
6. **Profil par échantillonnage (flamegraph)** — `perf record -g` puis
   `perf script` replié en piles si `perf_event_paranoid ≤ 1`, sinon
   échantillonnage gdb (`SIGUSR2`, script fourni par l'extension) ; build sans
   `--release` pour garder les symboles ; flamegraph en webview, clic vers la
   fonction Ocara.
7. **Appels système** — tableau de `strace -f -c` ; lancé à deux tailles,
   signale un appel proportionnel à N (`rt_sigprocmask` × N : exceptions ;
   `clone3` × N : une tâche async = un thread).
8. **Serveurs** — plateau mémoire guidé (séries de requêtes, `VmRSS` relevé
   dans `/proc/<pid>/status`) ; vidage du profil et du bilan de fuites sur
   `SIGUSR1` pour les programmes qui ne se terminent pas (CLI + runtime).
9. **ocaraunit** — `--leak-check` et `--profile` sur une suite de tests,
   agrégés par test ; CodeLens « Profiler ce test ».
10. **Historique exportable** — CSV/JSON avec date, genre, valeur, charge et
    commit (`git rev-parse --short HEAD`) ; seuil de régression avec alerte.
11. Arguments, variables d'environnement et entrée standard configurables ;
    tour de chauffe ; mesures annulables.

## Priorité / Complexité

**Moyenne** (fonctionnalités) — **Légère** à **Structurel** (4, 6 et 8
touchent le runtime ou le CLI).

## Fichiers clés

`tools/highlight/vsode/src/benchmark.ts`, `tools/highlight/vsode/package.json`,
`runtime/src/profile.rs`, `src/ir/profile.rs`, `src/core/leak_sites.rs`.
