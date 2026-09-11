# Trous de documentation et de diagnostics sur le modèle mémoire

## Documentation

- `docs/EBNF.md` §9 documente `var`/`scoped`/`consumed` mais ne dit **jamais explicitement** que `var` (le mot-clé par défaut) ne libère rien.
- Seule mention de "GC" dans tout le fichier : "pas de GC **imposé**" (docs/EBNF.md:59) — formulation ambiguë qui laisse penser à un GC optionnel, alors qu'il n'y en a aucun.
- `docs/workflow-compilation.md` (le seul document d'architecture du projet) ne mentionne à aucun moment la phase où les destructeurs `scoped`/`consumed` sont insérés, ni les diagnostics E17-E19.
- `README.md` et `docs/README.md` : aucune mention du modèle mémoire (pas de section "pas de GC", pas d'avertissement).

## Diagnostics manquants

Seuls E17 (`consumed` réutilisée), E18 (échappement d'une ressource) et E19 (`Thread` non finalisée) touchent la mémoire. Aucun diagnostic n'existe pour : un double-free explicite, un use-after-free d'une `scoped` après fin de bloc, la fuite d'un handle natif (`Mutex`, `SQLite`, `HTTPRequest`...) déclaré en `var` (silence total), ou la fuite du champ non pris en charge d'une classe.

## Ampleur

Documentation : travail léger (rédiger une section dédiée dans l'EBNF + un paragraphe dans workflow-compilation.md + un avertissement dans le README). Nouveaux diagnostics : travail moyen (nécessite d'abord que les mécanismes de suivi correspondants existent, cf. les autres fiches mémoire).
