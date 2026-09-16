# Roadmap Ocara

_Dernière mise à jour : 2026-09-16_

Ce document liste ce qu'il reste à faire pour faire d'Ocara un langage solide. Cette révision fait suite à une analyse complète du projet (doc, code source du compilateur, exemples, runtimes natifs) et **reprioritise délibérément autour de la robustesse, la stabilité et la fiabilité** — avant toute nouvelle fonctionnalité ou tout chantier de portage. Le focus reste, comme avant, la **gestion mémoire** : le compilateur n'a pas de ramasse-miettes (choix assumé et définitif), et l'historique du projet montre plusieurs SEGFAULTs confirmés par reproduction sur la représentation `mixed` — mais la même question de fiabilité se pose aussi sur le mécanisme d'exceptions (`setjmp`/`longjmp`), sur la quasi-absence de tests Rust unitaires en dehors du front-end, et sur au moins une race condition documentée et non corrigée (`HTTPServer`).

Ce fichier ne contient volontairement **aucun détail technique**. Chaque point renvoie vers une fiche dans [`docs/roadmap.d/`](roadmap.d/) pour l'implémentation, les fichiers concernés et les extraits de reproduction. À mettre à jour au fil des avancées : un point traité doit être retiré et sa fiche technique mise à jour ou supprimée. L'historique complet des correctifs déjà faits vit dans `git log`, pas dans ce fichier.

## Définition : « le langage est stable »

Cette roadmap est construite pour qu'on puisse dire que le langage est stable **quand la section "Priorité Haute" ci-dessous est vide** — pas avant. Ce n'est pas un objectif séparé à suivre en plus des tickets : c'est littéralement ce que cette section représente. Volontairement, aucune checklist n'est dupliquée ici (le projet a déjà payé le prix d'une source de vérité dupliquée ailleurs — voir `docs/adding-builtins.md`, la double liste `OCARA_BUILTINS`) : la liste unique à vider est celle de la section "Priorité Haute".

Ce que ça couvre concrètement, une fois les deux points de cette section clos : le mécanisme d'exceptions ne fait plus fuir silencieusement mémoire/ressources sans qu'au minimum ce soit détecté à la compilation ; et une décision consciente est prise sur la protection native de `HTTPServer` (ou son absence assumée, documentée et testée). (La représentation `mixed`, la duplication statique/sucre du type des paramètres, la couverture de tests Rust unitaires sur l'analyse d'échappement/ownership/boxing, la recapture d'une fermeture imbriquée dans une fermeture, le SEGFAULT sur une fermeture créée dans un bloc `if`/`switch`, et la re-promotion d'une fermeture à chaque itération d'une boucle ont déjà été traitées : voir [memoire-boxing-durcissement](roadmap.d/memoire-boxing-durcissement.md), [qualite-parite-sucre-statique-param-types](roadmap.d/qualite-parite-sucre-statique-param-types.md), [qualite-tests-unitaires-critiques](roadmap.d/qualite-tests-unitaires-critiques.md), [langage-nested-closure-recapture](roadmap.d/langage-nested-closure-recapture.md), [langage-closure-promotion-block-scope](roadmap.d/langage-closure-promotion-block-scope.md) et [langage-closure-promotion-in-loop](roadmap.d/langage-closure-promotion-in-loop.md), clos.)

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

Bloque la fiabilité du langage — à traiter avant toute nouvelle fonctionnalité. Voir « Définition : le langage est stable » ci-dessus : cette section vide = le langage est stable.

- **Dette transversale `setjmp`/`longjmp`** (un `raise` qui traverse un `try` fait fuir mémoire/ressources, au moins trois sous-systèmes concernés). Nouveau diagnostic W04 fait (`docs/diagnostics.md`) : rend le risque visible à la compilation pour toute `scoped`/`consumed` ressource (Mutex/SQLite/MySQL/MariaDB/Thread) — ne le corrige pas ; décision encore à prendre sur la généralisation du patron `withLock` (déjà fait pour `Mutex`) aux autres types ressource. *(Structurel si retenue)* → [détails](roadmap.d/exceptions-setjmp-longjmp-dette.md)
- **Race condition de `HTTPServer`** (captures partagées entre handlers non protégées par mutex). Documentation + exemple + test de charge concurrente faits (`docs/builtins/HTTPServer.md`, `examples/builtins/httpserver.oc`/`.sh`) ; décision encore à prendre sur une protection native (sérialiser l'invocation des handlers) vs rester sur le modèle "Mutex explicite" déjà utilisé par `Thread`. *(Structurel)* → [détails](roadmap.d/runtime-httpserver-race-condition.md)

---

## Priorité Moyenne

À traiter mais non bloquant pour la stabilité du langage.

_Vide pour l'instant — le seul point qui s'y trouvait (`-no-pie` au lien final) est clos, voir [securite-lien-no-pie](roadmap.d/securite-lien-no-pie.md)._

---

## Priorité Basse

Confort ou portée future — n'affecte pas la correction du compilateur ou des binaires produits.

- **Cohérence interne de l'EBNF et de la stdlib** (grammaire "complète" en contradiction avec sa propre section §2, exemples utilisant une syntaxe `Function` documentée comme supprimée, sémantiques divergentes entre builtins jumeaux — `String::replace` vs `Regex::replace`, `queryOne` SQLite vs MySQL). *(Simple)* → [détails](roadmap.d/coherence-documentation-ebnf-stdlib.md)

---

## Priorité Très Basse

Pas important du tout pour le moment — portage/intégration massifs, aucune urgence.

- **Vraie infrastructure CI pour MySQL** (service dans un futur pipeline CI, aucun aujourd'hui). *(Légère — pour plus tard)* → [détails](roadmap.d/qualite-couverture-tests.md)
- **Finaliser l'intégration Tauri** (aujourd'hui simulation en mémoire pour `listen`/`emit`/`dialog`/`notify`). *(Massive)* → [détails](roadmap.d/builtins-tauri.md)
- **Vérifier le round-trip complet "zéro `.a` → binaire fonctionnel"** en une seule commande — tentative abandonnée après plus d'une heure sans sortie visible (recompilation vendored OpenSSL/SQLite, ou blocage — indiscernable sans progression affichée). *(Légère — vérification, prévoir un budget de temps important et un moyen de surveiller la progression réelle)* → [détails](roadmap.d/packaging-build-cargo.md)
- **Étudier un vrai support Windows** pour la compilation du compilateur lui-même. *(Massive)* → [détails](roadmap.d/packaging-windows.md)
- **Étudier un vrai support Android** pour la compilation du compilateur lui-même. *(Massive)* → [détails](roadmap.d/packaging-android.md)

---

## Méthode de travail

* On analyse la roadmap et les fichiers `roadmap.d/` associés à un ticket en cours.
* On analyse les documentations
    * Workflow, l'EBNF et les documentations lié à notre ticket
* On utilise le Makefile
* On effectue les corrections et améliorations demandées.
* Si changement de syntaxe ou ajout:
    * On mets à jour l'extension vscode dans tools/ si c'est nécessaire
        * On lis le README.md
        * On desinstalle l'extension en cours
        * On recompile la mise à jour sans changer la version
        * On réinstalle l'extension
    * On mets à jour ocaracs si c'est nécessaire
        * On lis le README.md
        * On clean
        * On effectue les modifications
        * On compile
    * On mets à jour ocaraunit si c'est nécessaire
        * On lis le README.md
        * On clean
        * On effectue les modifications
        * On compile
* On crée un exemple pour les tests de régression si nécessaire.
* On crée un test unitaire si nécessaire.
* On lance les test de regression et les tests unitaire.
* On met à jour la documentation si nécessaire.
* On met à jour la roadmap.
* On affiche une liste simple, sans détails, des travaux effectués afin de préparer le commit.

Si, durant les travaux, nous constatons des bugs ou d’autres points à traiter, nous évaluons s’il est possible de les intégrer au ticket en cours. Si ce n’est pas possible, nous ajoutons ces nouvelles tâches à la roadmap.

---

## Suivi

Ce document est mis à jour au fil des avancées : quand un point est traité, le retirer de la section correspondante et mettre à jour ou supprimer la fiche technique associée dans `docs/roadmap.d/`.
