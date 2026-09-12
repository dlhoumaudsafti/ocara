# Trous de documentation et de diagnostics sur le modèle mémoire

## ✅ Documentation — corrigée

- `docs/EBNF.md` §9.1 (`var`) précise maintenant explicitement qu'une variable `var` n'est **jamais** libérée (pas de ramasse-miettes) — auparavant seul le tableau §9.2 sur `scoped` documentait un vrai comportement de libération, `var` n'était jamais mentionné comme ne libérant rien.
- `docs/EBNF.md` §9.2 (`scoped`) documente désormais aussi la limite connue de l'échappement par argument (`scoped`/`consumed` passée en paramètre à une fonction qui la conserve) en plus de la limite déjà documentée sur `raise`/`longjmp` — voir [memoire-echappement-argument](memoire-echappement-argument.md) (non corrigée, seulement documentée).
- `docs/EBNF.md` §1 : "pas de GC imposé" (ambigu, laissait penser à un GC optionnel) reformulé en "aucun ramasse-miettes — jamais, par choix de design définitif".
- `docs/workflow-compilation.md` : nouvelle sous-section 4️⃣d décrivant la phase de lowering qui insère les libérations `scoped`/`consumed`, avec renvoi vers les diagnostics E17-E19 (qui n'étaient mentionnés nulle part dans le seul document d'architecture du projet).
- `README.md` : nouvelle puce dans "Caractéristiques" signalant l'absence de GC dès la page d'entrée du projet.

## Diagnostics manquants — toujours ouvert

Seuls E17 (`consumed` réutilisée), E18 (échappement d'une ressource) et E19 (`Thread` non finalisée) touchent la mémoire. Aucun diagnostic n'existe pour : un double-free explicite, un use-after-free d'une `scoped` après fin de bloc, la fuite d'un handle natif (`Mutex`, `SQLite`, `HTTPRequest`...) déclaré en `var` (silence total), ou la fuite du champ non pris en charge d'une classe.

## Ampleur (restante)

Nouveaux diagnostics : travail moyen — nécessite d'abord que les mécanismes de suivi correspondants existent (voir [memoire-double-free-et-fuites-scoped](memoire-double-free-et-fuites-scoped.md) et [memoire-strategie-var](memoire-strategie-var.md)), pas un simple ajout de message d'erreur.
