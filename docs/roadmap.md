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

## Priorité Moyenne

### Langage

- **Instruction `emit`** (même rôle que `yield` en PHP) et type `message<T>` — **conception entièrement actée, parsing et sema construits, reste le lowering**. *(Massive)* → [détails](roadmap.d/langage-emit-iterable.md)
  1. ✅ Parsing (`emit`, `Type::Message`) — grammaire EBNF pas encore mise à jour (fait à l'étape 7)
  2. ✅ Sema (typage, diagnostics E30–E34, `message<T>` jamais nommable, règle "au plus un `emit` hors boucle" pour la consommation scalaire)
  3. Lowering — transformation en machine à états
  4. Lowering — les 3 formes de consommation (`for`, scalaire directe, `Array::fromMessage`) + libération (dont `break`/`return` anticipé)
  5. Lowering — `emit` dans un `try` (rejeu des `setjmp` à la reprise)
  6. Runtime (`Array::fromMessage`)
  7. Documentation (EBNF, workflow-compilation, adding-types)
  8. Tests de régression

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

* On analyse la roadmap et les fichiers `roadmap.d/` associés à la tâche en cours.
* On effectue les corrections et améliorations demandées.
* Si changement de syntaxe ou ajout:
    * On mets à jour l'extension vscode dans tools/ si c'est nécessaire
        * On desinstalle l'extension en cours
        * On recompile la mise à jour sans changer la version
        * On réinstalle l'extension
    * On mets à jour ocaracs si c'est nécessaire
    * On mets à jour ocaraunit si c'est nécessaire
* On crée un exemple pour les tests de régression si nécessaire.
* On crée un test unitaire si nécessaire.
* On lance les test de regression et les tests unitaire.
* On met à jour la documentation si nécessaire.
* On met à jour la roadmap.
* On affiche une liste simple, sans détails, des travaux effectués afin de préparer le commit.

Si, durant les travaux, nous constatons des bugs ou d’autres points à traiter, nous évaluons s’il est possible de les intégrer à la séance en cours. Si ce n’est pas possible, nous ajoutons ces nouvelles tâches à la roadmap.

---

## Suivi

Ce document est mis à jour au fil des avancées : quand un point est traité, le retirer de la section correspondante et mettre à jour ou supprimer la fiche technique associée dans `docs/roadmap.d/`.
