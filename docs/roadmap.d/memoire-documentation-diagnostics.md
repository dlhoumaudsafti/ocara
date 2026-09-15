# Diagnostics de fuite mémoire manquants pour `var`

La documentation du modèle mémoire est à jour, et le diagnostic de double-free explicite sur une ressource (E25) est corrigé — voir git log.

## Reste à faire : deux diagnostics de fuite, désormais possibles

Deux manques identifiés dès le départ restent non traités :
- **Fuite d'un handle natif déclaré en `var`** (`var m:Mutex = use Mutex()` jamais fermé, jamais détecté) ;
- **Fuite d'un champ de classe non pris en charge** par l'analyse de possession actuelle.

Ces deux diagnostics étaient bloqués faute d'une vraie stratégie de suivi mémoire pour `var` — **ce chantier est maintenant fait** (analyse d'échappement statique, libération automatique d'un `var` prouvé non-échappant, voir git log). Les deux diagnostics restent donc à écrire, mais ne sont plus bloqués par un prérequis manquant : l'infrastructure (`src/sema/escape.rs`, `register_owned_local`) existe déjà et pourrait être étendue pour repérer ces deux cas.

## Fichiers clés

`src/sema/escape.rs`, `src/sema/typecheck.rs`, `src/sema/error.rs`, `src/lower/stmt.d/ownership.rs`.
