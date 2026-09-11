# Roadmap Ocara

_Dernière mise à jour : 2026-09-11_

Ce document liste ce qu'il reste à faire pour faire d'Ocara un langage solide, avec un focus prioritaire sur la **gestion mémoire** : le compilateur n'a pas de ramasse-miettes (choix assumé et définitif), mais rien aujourd'hui ne garantit l'absence de fuites, de doubles libérations ou de corruptions mémoire silencieuses.

Ce fichier ne contient volontairement **aucun détail technique**. Chaque point renvoie vers une fiche dans [`docs/roadmap.d/`](roadmap.d/) pour l'implémentation, les fichiers concernés et les extraits de reproduction. À mettre à jour au fil des avancées : un point traité doit être retiré (ou déplacé dans une section "Fait") et sa fiche technique mise à jour ou supprimée.

## Légende

**Priorité** — Haute : bloque la fiabilité du langage · Moyenne : à traiter mais non bloquant · Basse : confort ou portée future

**Complexité** :
- **Simple** — correctif isolé et mécanique, peu de risque
- **Légère** — travail limité en volume, circonscrit à quelques fichiers
- **Structurel** — demande de repenser une partie de l'architecture existante
- **Massive** — gros volume de travail ou fonctionnalité entièrement à construire
- **Dangereuse** — touche une zone sensible du compilateur/runtime où une erreur peut tout casser silencieusement (mémoire, concurrence) ; à traiter avec prudence et de bons tests de non-régression

---

## Priorité Haute

### Gestion mémoire

- **Combler la faille d'échappement des variables `scoped`/`consumed` passées en argument** — une valeur possédée peut être stockée ailleurs sans être clonée ; corruption mémoire confirmée. *(Dangereuse)* → [détails](roadmap.d/memoire-echappement-argument.md)
- **Résoudre les blocages provoqués par `raise` traversant un verrou tenu** (SQLite/MySQL, `Mutex`) — un `raise` peut sauter un déverrouillage et bloquer le programme indéfiniment. *(Dangereuse)* → [détails](roadmap.d/memoire-deadlocks-raise.md)
- **Éliminer les doubles libérations, use-after-free et fuites autour de `scoped`/`consumed`** — fermeture manuelle + destruction automatique, réutilisation en boucle, réaffectation, shadowing. *(Structurel)* → [détails](roadmap.d/memoire-double-free-et-fuites-scoped.md)
- **Sécuriser la mémoire partagée entre threads et closures** — aucune synchronisation aujourd'hui sur les variables capturées. *(Structurel)* → [détails](roadmap.d/memoire-concurrence-threads.md)
- **Concevoir une vraie stratégie de gestion mémoire pour `var`** (et pour les environnements de closures) — le mot-clé par défaut du langage ne libère aujourd'hui jamais rien. *(Massive)* → [détails](roadmap.d/memoire-strategie-var.md)

### Langage

- **Étendre le typage aux valeurs génériques** — un appel de méthode invalide sur une valeur générique (`List<T>`) n'est aujourd'hui pas détecté. *(Structurel)* → [détails](roadmap.d/langage-generiques.md)
- **Corriger la perte des interfaces implémentées transitivement à l'import** et vérifier la signature d'une méthode d'interface. *(Légère)* → [détails](roadmap.d/langage-interfaces.md)

### Builtins

- **Rendre cohérente la remontée d'erreurs des builtins** (MySQL et YAML déclarent une exception qu'ils ne lèvent jamais ; DotEnv échoue en silence). *(Simple)* → [détails](roadmap.d/builtins-erreurs-incoherentes.md)

---

## Priorité Moyenne

### Gestion mémoire

- **Robustifier les libérations bas niveau du runtime** (tag d'exception ambigu, taille recalculée sans header fiable, détection de type par heuristique sur un entier). *(Dangereuse)* → [détails](roadmap.d/memoire-fiabilite-runtime-bas-niveau.md)
- **Documenter le modèle mémoire et étoffer les diagnostics associés** (aucune section dédiée dans l'EBNF ni dans le guide de compilation, pas de diagnostic pour une fuite de handle natif). *(Légère)* → [détails](roadmap.d/memoire-documentation-diagnostics.md)

### Langage

- **Vérifier l'arité et les contraintes des paramètres de type génériques.** *(Légère)* → [détails](roadmap.d/langage-generiques.md)
- **Donner une existence réelle aux interfaces à l'exécution** (dispatch dynamique, aujourd'hui purement statique). *(Massive)* → [détails](roadmap.d/langage-interfaces.md)
- **Simplifier la résolution des imports** (deux chemins redondants et incohérents pour l'ancien format d'import). *(Structurel)* → [détails](roadmap.d/langage-imports-modules.md)
- **Renforcer les vérifications de `try`/`on`/`raise`** (classe inconnue non détectée, ordre du catch-all non imposé, pas de hiérarchie d'exceptions réelle). *(Structurel)* → [détails](roadmap.d/langage-exceptions.md)

### Builtins

- **Achever le support YAML** (flottants perdus silencieusement, types "tagged" non supportés). *(Légère)* → [détails](roadmap.d/builtins-yaml.md)

### Build & portabilité

- **Rendre le compilateur constructible nativement avec Cargo** (aujourd'hui, `build.rs` exige une orchestration Makefile préalable). *(Structurel)* → [détails](roadmap.d/packaging-build-cargo.md)

### Qualité

- **Étendre la CI/régression aux dossiers actuellement hors périmètre** (`examples/generics`, `examples/from`, `examples/mods`, `examples/advanced`) et réactiver de vraies assertions sur le test des interfaces. *(Légère)* → [détails](roadmap.d/qualite-couverture-tests.md)

---

## Priorité Basse

### Langage

- **Mettre à jour les exemples utilisant une syntaxe obsolète** (`T[]`, `==`) et corriger les décalages doc/exemples restants. *(Simple)* → [détails](roadmap.d/langage-syntaxe-obsolete.md)

### Builtins

- **Finaliser l'intégration Tauri** (aujourd'hui simulation en mémoire pour `listen`/`emit`/`dialog`/`notify`). *(Massive)* → [détails](roadmap.d/builtins-tauri.md)
- **Implémenter `Map::forEach`** (callback jamais appelé aujourd'hui, seul TODO fonctionnel du runtime). *(Simple)*

### Build & portabilité

- **Étudier un vrai support Windows** pour la compilation du compilateur lui-même. *(Massive)* → [détails](roadmap.d/packaging-windows.md)
- **Fiabiliser les dépendances système de build** (OpenSSL imposé systématiquement, shim pkg-config ad hoc, contrainte `-j1`). *(Légère)* → [détails](roadmap.d/packaging-build-cargo.md)

### Qualité

- **Nettoyer le dépôt d'exemples** (fichiers `.bak` obsolètes, artefacts binaires committés). *(Simple)*

---

## Suivi

Ce document est mis à jour au fil des avancées : quand un point est traité, le retirer de la section correspondante et mettre à jour ou supprimer la fiche technique associée dans `docs/roadmap.d/`.
