# Roadmap Ocara

_Dernière mise à jour : 2026-09-11_

Ce document liste ce qu'il reste à faire pour faire d'Ocara un langage solide, avec un focus prioritaire sur la **gestion mémoire** : le compilateur n'a pas de ramasse-miettes (choix assumé et définitif), mais rien aujourd'hui ne garantit l'absence de fuites, de doubles libérations ou de corruptions mémoire silencieuses.

Ce fichier ne contient volontairement **aucun détail technique**. Chaque point renvoie vers une fiche dans [`docs/roadmap.d/`](roadmap.d/) pour l'implémentation, les fichiers concernés et les extraits de reproduction. À mettre à jour au fil des avancées : un point traité doit être retiré (ou déplacé dans une section "Fait") et sa fiche technique mise à jour ou supprimée.

## Fait récemment

- ✅ **Cohérence de la remontée d'erreurs des builtins** — `MySQLException` (code 101/102/103) est maintenant réellement levée par `MySQL_connect`/`execute`/`query`/`queryOne` (calqué sur `SQLiteException`), y compris pour `MariaDB`. La documentation `YAML.md`/`DotEnv.md` a été corrigée pour ne plus promettre une exception qui n'est pas levée (comportement volontairement inchangé pour ces deux modules — voir la fiche pour le détail de ce choix).
- ✅ **Interfaces implémentées transitivement à l'import** — `import Circle from "fichier"` rapatrie désormais aussi les interfaces référencées par `Circle.implements`, même sans les importer explicitement.
- ✅ **Vérification de signature d'interface (E09)** — un `implements` dont la méthode ne correspond pas en arité ou en type (paramètres/retour) est maintenant rejeté à la compilation, au lieu d'être accepté silencieusement.
- ✅ **Documentation du modèle mémoire** — `var` documenté comme ne libérant jamais rien, limite connue de l'échappement par argument ajoutée à côté de celle sur `raise`, formulation "pas de GC imposé" corrigée, nouvelle section dans `workflow-compilation.md` sur la phase d'insertion des libérations (+ renvoi vers E17-E19), mention dans le README.
- ✅ **Arité des génériques** — `use List<int,string,Foo>()` sur un `generic List<T>` (1 paramètre) est maintenant rejeté (nouveau diagnostic **E21**), au lieu de compiler silencieusement.
- ✅ **Support des flottants en YAML** — `YAML::decode`/`encode` reconstruisent maintenant un vrai flottant depuis/vers un nombre YAML à virgule, au lieu de silencieusement devenir `0`.
- ✅ **Bug d'import bloquant un fichier multi-classes** — `import Circle from "X"` puis `import Rectangle from "X"` (cas d'usage documenté par l'EBNF elle-même) ignorait silencieusement le second import ; le test `examples/tests/11_interfacesTest.oc` (cassé pour cette raison) compile et passe maintenant ses 4 assertions, `examples/from/import_from.oc` est ajouté à la CI, et `consumed` a maintenant un test dédié.

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

---

## Priorité Moyenne

### Gestion mémoire

- **Robustifier les libérations bas niveau du runtime** (tag d'exception ambigu, taille recalculée sans header fiable, détection de type par heuristique sur un entier). *(Dangereuse)* → [détails](roadmap.d/memoire-fiabilite-runtime-bas-niveau.md)
- **Ajouter des diagnostics mémoire dédiés** (double-free explicite, fuite d'un handle natif déclaré en `var`, fuite d'un champ de classe non pris en charge) — bloqué tant que les mécanismes de suivi correspondants n'existent pas (voir les items Haute priorité ci-dessus). *(Structurel)* → [détails](roadmap.d/memoire-documentation-diagnostics.md)

### Langage

- **Donner une existence réelle aux interfaces à l'exécution** (dispatch dynamique, aujourd'hui purement statique). *(Massive)* → [détails](roadmap.d/langage-interfaces.md)
- **Simplifier la résolution des imports** (deux chemins redondants et incohérents pour l'ancien format d'import — confirmé concrètement sur `examples/project/tests/mainTest.oc`). *(Structurel)* → [détails](roadmap.d/langage-imports-modules.md)
- **Renforcer les vérifications de `try`/`on`/`raise`** (classe inconnue non détectée, ordre du catch-all non imposé, pas de hiérarchie d'exceptions réelle). *(Structurel)* → [détails](roadmap.d/langage-exceptions.md)
- **Les littéraux `float`/`bool` dans un `array<mixed>`/`map<string, mixed>` sont stockés comme leur représentation string** au lieu d'être boxés — perte de type silencieuse touchant tout consommateur de `mixed` (`YAML::encode`, `JSON::encode`, ...), découvert en travaillant sur le support YAML. *(Structurel)* → [détails](roadmap.d/langage-mixed-literal-stringification.md)

### Build & portabilité

- **Rendre le compilateur constructible nativement avec Cargo** (aujourd'hui, `build.rs` exige une orchestration Makefile préalable). *(Structurel)* → [détails](roadmap.d/packaging-build-cargo.md)

### Qualité

- **Écrire des scripts CI dédiés pour les exemples serveur HTTP de `examples/advanced/`** (`httpserver`, `mini_project`, `tauri_httpserver`) — ces programmes bloquent indéfiniment (`server.start()`/`run()`), il faut le même mécanisme que `examples/builtins/httpserver.sh` (démarrage en fond + requête + arrêt) pour chacun. *(Légère)* → [détails](roadmap.d/qualite-couverture-tests.md)
- **Mettre en place une infrastructure CI pour MySQL** (service/container) pour que `builtins/mysql` cesse d'échouer faute de serveur local. *(Légère)* → [détails](roadmap.d/qualite-couverture-tests.md)

---

## Priorité Basse

### Langage

- **Mettre à jour les exemples utilisant une syntaxe obsolète** (`T[]`, `==`) et corriger les décalages doc/exemples restants. *(Simple)* → [détails](roadmap.d/langage-syntaxe-obsolete.md)

### Builtins

- **Finaliser l'intégration Tauri** (aujourd'hui simulation en mémoire pour `listen`/`emit`/`dialog`/`notify`). *(Massive)* → [détails](roadmap.d/builtins-tauri.md)
- **Implémenter `Map::forEach`** (callback jamais appelé aujourd'hui, seul TODO fonctionnel du runtime). *(Simple)*

### Build & portabilité

- **Étudier un vrai support Windows** pour la compilation du compilateur lui-même. *(Massive)* → [détails](roadmap.d/packaging-windows.md)
- **Étudier un vrai support Android** pour la compilation du compilateur lui-même. *(Massive)*
- **Fiabiliser les dépendances système de build** (OpenSSL imposé systématiquement, shim pkg-config ad hoc, contrainte `-j1`). *(Légère)* → [détails](roadmap.d/packaging-build-cargo.md)

### Qualité

- **Nettoyer le dépôt d'exemples** (fichiers `.bak` obsolètes, artefacts binaires committés). *(Simple)*

---

## Suivi

Ce document est mis à jour au fil des avancées : quand un point est traité, le retirer de la section correspondante et mettre à jour ou supprimer la fiche technique associée dans `docs/roadmap.d/`.
