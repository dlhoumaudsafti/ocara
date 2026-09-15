# Roadmap Ocara

_Dernière mise à jour : 2026-09-16_

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

- **Convergence structurelle de `expr_ir_type`** — `Array`/`Map` (bug réel corrigé) et `HTTPRequest`/`HTTPResponse` (bug réel corrigé plus tôt) ont chacun eu une divergence statique/sucre distincte ; `String`/`JSON` audités sans divergence trouvée. Les 6 builtins à double forme (`allows_instance_sugar`) ont maintenant des tests de parité (39 assertions, voir la fiche). Reste seulement la convergence de fond : faire résoudre `Expr::StaticCall`/`Expr::Call{Field}` par une seule fonction partagée dans `expr_ir_type`, pour éliminer structurellement ce risque de récidive. *(Structurel — à ne traiter qu'avec de bons tests de non-régression en place, déjà le cas)* → [détails](roadmap.d/qualite-parite-sucre-statique.md)
- **Ambiguïté `0`/`null` dans un `mixed`** — un entier brut `0` logé dans un `mixed` (jamais boxé, optimisation volontaire de `box_int_if_needed`) est structurellement indiscernable de `null` (les deux valent le bit pattern `0`) : `val_to_string`/`__val_to_str` (et donc tout code générique consommant un `mixed` — logs `UnitTest::assertEquals`, JSON/YAML potentiellement...) affichent "null" pour un entier 0 authentique. Découvert en corrigeant l'affichage `Array::get`/`Map::get` ci-dessous. Nécessite de revoir comment `null` est représenté (actuellement confondu avec l'entier 0) — changement de fond, à ne pas improviser. *(Structurel — touche le boxing/la représentation `mixed`)* → [détails](roadmap.d/langage-array-get-display-bug.md)

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
