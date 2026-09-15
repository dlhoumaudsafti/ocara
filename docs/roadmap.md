# Roadmap Ocara

_Dernière mise à jour : 2026-09-15_

Ce document liste ce qu'il reste à faire pour faire d'Ocara un langage solide, avec un focus prioritaire sur la **gestion mémoire** : le compilateur n'a pas de ramasse-miettes (choix assumé et définitif), mais rien aujourd'hui ne garantit l'absence de fuites, de doubles libérations ou de corruptions mémoire silencieuses.

Ce fichier ne contient volontairement **aucun détail technique**. Chaque point renvoie vers une fiche dans [`docs/roadmap.d/`](roadmap.d/) pour l'implémentation, les fichiers concernés et les extraits de reproduction. À mettre à jour au fil des avancées : un point traité doit être retiré et sa fiche technique mise à jour ou supprimée. L'historique complet des correctifs déjà faits vit dans `git log`, pas dans ce fichier.

## Légende

**Priorité** — Haute : bloque la fiabilité du langage · Moyenne : à traiter mais non bloquant · Basse : confort ou portée future · Très Basse : pas important du tout pour le moment

**Complexité** :
- **Simple** — correctif isolé et mécanique, peu de risque
- **Légère** — travail limité en volume, circonscrit à quelques fichiers
- **Structurel** — demande de repenser une partie de l'architecture existante
- **Massive** — gros volume de travail ou fonctionnalité entièrement à construire
- **Dangereuse** — touche une zone sensible du compilateur/runtime où une erreur peut tout casser silencieusement (mémoire, concurrence) ; à traiter avec prudence et de bons tests de non-régression

---

## Priorité Haute

### Mémoire

- **Deux diagnostics de fuite mémoire pour `var`** (handle natif jamais fermé, champ de classe non pris en charge) — désormais possibles, plus aucun prérequis manquant. *(Structurel)* → [détails](roadmap.d/memoire-documentation-diagnostics.md)

### Langage

- **`self.méthode()`/`parent.méthode()` ne sont jamais dispatchés dynamiquement**, contrairement à un appel externe (`obj.méthode()`) — casse le patron "template method". *(Structurel)* → [détails](roadmap.d/langage-interfaces.md)

---

## Priorité Moyenne

### Langage

- **`extends` sur un `generic` n'est jamais vérifié sémantiquement** — un parent inexistant compile sans erreur. *(Légère)* → [détails](roadmap.d/langage-generiques.md)
- **`value_to_json`/`value_to_yaml` confondent un entier brut `0`/`1` avec un booléen** — l'heuristique n'a plus lieu d'être maintenant que le boxing des arguments `mixed` est corrigé. *(Simple)* → [détails](roadmap.d/langage-mixed-literal-stringification.md)
- **`IO::writeln(JSON::encode(x))` affiche un nombre incohérent sans variable intermédiaire** — `expr_ir_type` ne reconnaît pas `JSON_encode`/les méthodes d'instance comme retournant `Ptr`. *(Légère)* → [détails](roadmap.d/langage-mixed-literal-stringification.md)
- **Ajout d'une instruction `emit`** (même rôle que `yield` en PHP : suspend/reprend l'exécution en produisant une valeur à la fois) — une fonction qui l'utilise retourne un nouveau type `iterable`. *(Massive)* → [détails](roadmap.d/langage-emit-iterable.md)

### Mémoire / runtime

- **Une string à NUL interne reste tronquée à l'affichage/comparaison** (`ptr_to_str` basé sur `CStr::from_ptr`) — plus de risque de corruption, mais le contenu reste faux. *(Structurel)* → [détails](roadmap.d/memoire-fiabilite-runtime-bas-niveau.md)
- **Diagnostic mort `SemaError::OwnershipNotSupported`** — jamais émis en pratique, à activer ou retirer. *(Simple)* → [détails](roadmap.d/memoire-double-free-et-fuites-scoped.md)

---

## Priorité Basse

### Builtins

- **Décider si `YAMLException`/`DotEnvException` doivent un jour être réellement levées**, ou si ces classes orphelines doivent être retirées. *(Légère — décision de design)* → [détails](roadmap.d/builtins-erreurs-incoherentes.md)
- **`HTTPRequest`/`HTTPResponse` non typables `scoped`/`consumed`** (simples `int` aujourd'hui) — nécessiterait d'en faire un vrai type. *(Structurel)* → [détails](roadmap.d/memoire-double-free-et-fuites-scoped.md)

### Runtime bas niveau (mineur, non confirmé en pratique)

- **`UnitTest::assertContains` appelle `ptr_to_str` sans garde** sur ses arguments `mixed`. *(Simple)* → [détails](roadmap.d/memoire-fiabilite-runtime-bas-niveau.md)
- **Libération/clonage "shallow" d'un conteneur imbriqué à deux niveaux** (`array<array<int>>`) reste sur le chemin récursif générique au niveau externe. *(Dangereuse)* → [détails](roadmap.d/memoire-fiabilite-runtime-bas-niveau.md)

### Build & portabilité

- **Vérifier la contrainte `-j1` documentée pour Cranelift** — le `Makefile` utilise en réalité `-j4` aujourd'hui, à confirmer sur une machine où l'OOM avait été observé. *(Simple)* → [détails](roadmap.d/packaging-build-cargo.md)
- **Vérifier le round-trip complet "zéro `.a` → binaire fonctionnel"** en une seule commande. *(Simple — vérification)* → [détails](roadmap.d/packaging-build-cargo.md)

### Qualité / CI

- **`examples/generics/` toujours absent de toute cible Makefile/CI** (la syntaxe y est corrigée, mais rien ne les exécute). *(Simple)* → [détails](roadmap.d/qualite-couverture-tests.md)
- **Vraie infrastructure CI pour MySQL** (service dans un futur pipeline CI, aucun aujourd'hui). *(Légère — pour plus tard)* → [détails](roadmap.d/qualite-couverture-tests.md)

---

## Priorité Très Basse

Pas important du tout pour le moment — portage/intégration massifs, aucune urgence.

- **Finaliser l'intégration Tauri** (aujourd'hui simulation en mémoire pour `listen`/`emit`/`dialog`/`notify`). *(Massive)* → [détails](roadmap.d/builtins-tauri.md)
- **Étudier un vrai support Windows** pour la compilation du compilateur lui-même. *(Massive)* → [détails](roadmap.d/packaging-windows.md)
- **Étudier un vrai support Android** pour la compilation du compilateur lui-même. *(Massive)* → [détails](roadmap.d/packaging-android.md)

---

## Méthode de travail

* On analyse la roadmap et les fichiers `roadmap.d/` associés à la tâche en cours.
* On effectue les corrections et améliorations demandées.
* On met à jour la documentation si nécessaire.
* On crée un exemple pour les tests de régression si nécessaire.
* On crée un test unitaire si nécessaire.
* On met à jour la roadmap.
* On affiche une liste simple, sans détails, des travaux effectués afin de préparer le commit.

Si, durant les travaux, nous constatons des bugs ou d’autres points à traiter, nous évaluons s’il est possible de les intégrer à la séance en cours. Si ce n’est pas possible, nous ajoutons ces nouvelles tâches à la roadmap.


---

## Suivi

Ce document est mis à jour au fil des avancées : quand un point est traité, le retirer de la section correspondante et mettre à jour ou supprimer la fiche technique associée dans `docs/roadmap.d/`.
