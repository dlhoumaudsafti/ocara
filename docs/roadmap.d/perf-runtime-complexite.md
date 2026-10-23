# Performances : pistes d'optimisation restantes

Audit du 2026-10-19, mis à jour le 2026-10-23. Les 34 mesures de `make perf`
et celles de `make perf-extra` (démarrage, mémoire, serveur HTTP) sont
meilleures que Python, PHP, Perl — tableau dans `docs/benchmarking.md`. Ce qui reste,
pour gagner encore :

1. **Locales encore en mémoire** : une locale dont l'adresse ne sert qu'à
   ses lectures et écritures est une `Variable` SSA de Cranelift (registre).
   Restent en pile : les locales d'une fonction qui appelle `setjmp`
   (générateurs — un registre ne survit pas à `longjmp`) et celles dont
   l'adresse est transmise (cellules de capture des `nameless` et des
   `try`). Mesuré le 2026-10-23 sur 5·10⁷ tours : la boucle d'un générateur
   n'en paie rien (0,23 s contre 0,24 s hors générateur) ; une variable
   capturée par un `nameless` coûtait deux appels au runtime par accès
   (0,34 s contre 0,20 s) — la cellule est désormais lue et écrite en ligne
   tant qu'un seul thread exécute du code Ocara (0,25 s, `clos` 0,05 →
   0,04 s). Reste le chargement du drapeau multi-thread à chaque accès.
   `opt_level = speed` essayé le 2026-10-21 après le passage en registres :
   aucun gain (boucle `loop` 0,06 → 0,09 s), réglage par défaut conservé.
2. **Retenue des paramètres et de `self`** : une méthode ou une fonction ne
   retient plus `self` quand rien dans son corps ne peut le libérer, ni ses
   paramètres quand son corps est pur (aucune écriture de champ compté ou
   de conteneur, seulement des appels purs : méthode d'une locale typée
   dont toutes les définitions sont pures, fonction pure, `Math`,
   `String`, conversions scalaires de `Convert`) — `meth` 0,15 → 0,01 s,
   `calls` 0,37 → 0,03 s. Restent retenus : `self` d'une méthode qui
   appelle un builtin à effet, une fermeture ou la méthode d'un receveur de
   type inconnu (`mixed`, interface, champ), et les paramètres d'un corps
   qui écrit un champ compté. Une preuve côté appelant (receveur tenu par
   une locale non capturée) changerait la convention d'appel de tous les
   chemins qui entrent dans une méthode (liaison dynamique, rappels du
   runtime, `async`) : un seul oubli serait une libération prématurée.

## Écarté

- **Retour au mode mono-thread après la fin des threads** : les comptes de
  références redeviennent atomiques dès qu'un thread Ocara a existé
  (≈ 10 ns par opération, `mt` 0,09 s contre 0,53 s pour Python). Revenir
  en arrière n'est pas sûr : des threads du runtime (ouvriers du serveur
  HTTP, pool des tâches `async`, rappels Tauri) entrent dans du code Ocara
  sans passer par le thread qui crée ; un passage en mode non atomique
  pendant une entrée corromprait des comptes.

## Priorité / Complexité

**Basse** (aucune mesure plus lente que Python, PHP, Perl) — **Structurel** (preuve
côté appelant, locales capturées hors cellule).

## Fichiers clés

`src/codegen/emit.d/value_slots.rs`, `src/codegen/emit.d/instructions.d/calls.rs`
(`emit_cell_access`), `src/lower/builder.d/self_borrow.rs`,
`src/lower/expr.d/lower.rs` (promotion des captures), `ci/perf_compare.sh`.
