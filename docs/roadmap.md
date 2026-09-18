# Roadmap Ocara

_Dernière mise à jour : 2026-09-18_

Ce document liste ce qu'il reste à faire pour faire d'Ocara un langage solide. Cette révision fait suite à une analyse complète du projet (doc, code source du compilateur, exemples, runtimes natifs) et **reprioritise délibérément autour de la robustesse, la stabilité et la fiabilité** — avant toute nouvelle fonctionnalité ou tout chantier de portage. Le focus reste, comme avant, la **gestion mémoire** : le compilateur n'a pas de ramasse-miettes (choix assumé et définitif), et l'historique du projet montre plusieurs SEGFAULTs confirmés par reproduction sur la représentation `mixed` — mais la même question de fiabilité se posait aussi sur le mécanisme d'exceptions (`setjmp`/`longjmp`), sur la quasi-absence de tests Rust unitaires en dehors du front-end, et sur au moins une race condition documentée (`HTTPServer`).

Ce fichier ne contient volontairement **aucun détail technique**. Chaque point renvoie vers une fiche dans [`docs/roadmap.d/`](roadmap.d/) pour l'implémentation, les fichiers concernés et les extraits de reproduction. À mettre à jour au fil des avancées : un point traité doit être retiré et sa fiche technique mise à jour ou supprimée. L'historique complet des correctifs déjà faits vit dans `git log`, pas dans ce fichier.

## Définition : « le langage est stable »

Cette roadmap est construite pour qu'on puisse dire que le langage est stable **quand la section "Priorité Haute" ci-dessous est vide** — pas avant. Ce n'est pas un objectif séparé à suivre en plus des tickets : c'est littéralement ce que cette section représente. Volontairement, aucune checklist n'est dupliquée ici (le projet a déjà payé le prix d'une source de vérité dupliquée ailleurs — voir `docs/adding-builtins.md`, la double liste `OCARA_BUILTINS`) : la liste unique à vider est celle de la section "Priorité Haute".

**La section "Priorité Haute" a de nouveau un point ouvert** (voir ci-dessous) — au sens de cette définition, le langage n'est momentanément plus "stable". Plusieurs points l'ont déjà été et le resteront quoi qu'il arrive : la représentation `mixed`, la duplication statique/sucre du type des paramètres, la couverture de tests Rust unitaires sur l'analyse d'échappement/ownership/boxing, la recapture d'une fermeture imbriquée, le SEGFAULT sur une fermeture créée dans un bloc `if`/`switch`, la re-promotion d'une fermeture à chaque itération d'une boucle, la dette `setjmp`/`longjmp`, la race condition de `HTTPServer`, et `use Classe(...).méthode()`/`HTTPRequest::méthode(...).méthode()` chaînés (résultat silencieusement faux, ou pour `Thread::run`, jamais lancé du tout) ont toutes été traitées : voir [memoire-boxing-durcissement](roadmap.d/memoire-boxing-durcissement.md), [qualite-parite-sucre-statique-param-types](roadmap.d/qualite-parite-sucre-statique-param-types.md), [qualite-tests-unitaires-critiques](roadmap.d/qualite-tests-unitaires-critiques.md), [langage-nested-closure-recapture](roadmap.d/langage-nested-closure-recapture.md), [langage-closure-promotion-block-scope](roadmap.d/langage-closure-promotion-block-scope.md), [langage-closure-promotion-in-loop](roadmap.d/langage-closure-promotion-in-loop.md), [exceptions-setjmp-longjmp-dette](roadmap.d/exceptions-setjmp-longjmp-dette.md), [runtime-httpserver-race-condition](roadmap.d/runtime-httpserver-race-condition.md) et [langage-use-chaine-valeur-retour-perdue](roadmap.d/langage-use-chaine-valeur-retour-perdue.md), toutes closes. Les sections Moyenne/Basse/Très Basse ci-dessous restent à traiter mais ne bloquent pas cette définition — voir la Légende.

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

- **`import Classe as Alias` (classe UTILISATEUR, pas un builtin `ocara.*`) casse la résolution de méthode héritée dès qu'un AUTRE fichier référence la même classe par son vrai nom** — résultat silencieusement faux (méthode jamais appelée), aucune erreur de compilation. Reproduit dans `examples/advanced/tauri_httpserver` (`import configs.Server as HTTP` empêchait TOUTE route d'être enregistrée, `server.route(...)` hérité de `HTTPServer` ne se résolvait plus). Contournement appliqué (ne pas aliaser), pas encore corrigé dans le compilateur. *(Structurel — touche la fusion multi-fichiers des imports et potentiellement l'identité runtime des classes, deux pistes de design non tranchées)* → [détails](roadmap.d/langage-alias-classe-utilisateur-heritage-casse.md)
- **Chaîner un appel de méthode sur le résultat `void` d'un appel précédent (`self.port(8080).workers(4)`) compile sans erreur, et la suite de la chaîne est silencieusement ignorée** — reproduit dans `examples/advanced/tauri_httpserver/configs/Server.oc` (`.workers(4)`/`.rootPath(...)` jamais exécutés, aucun `root_path` jamais configuré côté runtime). Contournement appliqué (appels séparés), pas encore corrigé. *(Complexité non évaluée — probablement une vérification sema absente, pas encore localisée)* → [détails](roadmap.d/langage-appel-methode-sur-void-accepte.md)

---

## Priorité Moyenne

À traiter mais non bloquant pour la stabilité du langage.

_Vide pour l'instant — les points qui s'y trouvaient sont clos : `SQLite`/`MySQL`/`MariaDB` (requêtes paramétrées + transactions, voir [stdlib-sqlite-requetes-parametrees-transactions](roadmap.d/stdlib-sqlite-requetes-parametrees-transactions.md) et [stdlib-mysql-requetes-parametrees-transactions](roadmap.d/stdlib-mysql-requetes-parametrees-transactions.md)), `-no-pie` au lien final (voir [securite-lien-no-pie](roadmap.d/securite-lien-no-pie.md) ; sa suite différée est suivie en Priorité Très Basse ci-dessous), et permettre une `property` de type ressource sur une classe utilisateur (analyse d'échappement étendue aux classes composites, voir [langage-destructeur-champ-ressource](roadmap.d/langage-destructeur-champ-ressource.md))._

---

## Priorité Basse

Confort ou portée future — n'affecte pas la correction du compilateur ou des binaires produits.

- **Réflexion (non tranchée) : remplacer `for x in a..b` par `for x in 1 to 10` (borne incluse) / `for x in 1 until 10` (borne exclue)** — la borne de fin exclue de `..` n'est pas lisible au point d'appel ; à peser contre le coût d'un changement de syntaxe cassant sur tout le corpus existant. *(Structurel si retenu — voir la fiche pour la discussion complète avant tout engagement)* → [détails](roadmap.d/reflexion-syntaxe-for-range.md)
- **Réflexion (non tranchée) : `for x in myarray when x greater|smaller|(not) equal| literral|var|scoped|consumed|const|property {}` et `for x when x greater|smaller|(not) equal| literral|var|scoped|consumed|const|property {}` et `for x=1 when x greater|smaller|(not) equal| literral|var|scoped|consumed|const|property {}`**

(Quatre points qui se trouvaient ici sont clos : la pédagogie en retard sur le corpus `advanced/` — `parent::` documenté en EBNF §18.1 et démontré dans `12_inheritance.oc`, pointeur ajouté vers EBNF §5 pour les blocs runtime, ligne `32_strict_operators.oc` corrigée, voir [documentation-pedagogie-en-retard](roadmap.d/documentation-pedagogie-en-retard.md) — l'extension VSCode alignée sur la version 1.0.0 du langage, voir [outillage-vscode-version-bump](roadmap.d/outillage-vscode-version-bump.md) — la cohérence interne de l'EBNF et de la stdlib, voir [coherence-documentation-ebnf-stdlib](roadmap.d/coherence-documentation-ebnf-stdlib.md) — et l'indexation chaînée array-puis-map (`arr[0]["name"]` retournait silencieusement `null`), corrigée par une résolution récursive du type d'une indexation supportant une profondeur arbitraire, voir [langage-index-chaine-sur-map](roadmap.d/langage-index-chaine-sur-map.md).)

---

## Priorité Très Basse

Pas important du tout pour le moment — portage/intégration massifs, aucune urgence.

- **Vraie infrastructure CI pour MySQL** (service dans un futur pipeline CI, aucun aujourd'hui). *(Légère — pour plus tard)* → [détails](roadmap.d/qualite-couverture-tests.md)
- **Finaliser l'intégration Tauri** (aujourd'hui simulation en mémoire pour `listen`/`emit`/`dialog`/`notify`). *(Massive)* → [détails](roadmap.d/builtins-tauri.md)
- **Vérifier le round-trip complet "zéro `.a` → binaire fonctionnel"** en une seule commande — tentative abandonnée après plus d'une heure sans sortie visible (recompilation vendored OpenSSL/SQLite, ou blocage — indiscernable sans progression affichée). *(Légère — vérification, prévoir un budget de temps important et un moyen de surveiller la progression réelle)* → [détails](roadmap.d/packaging-build-cargo.md)
- **Étudier un vrai support Windows** pour la compilation du compilateur lui-même. *(Massive)* → [détails](roadmap.d/packaging-windows.md)
- **Étudier un vrai support Android** pour la compilation du compilateur lui-même. *(Massive)* → [détails](roadmap.d/packaging-android.md)
- **Suite différée de `securite-lien-no-pie` : activer `is_pic` côté Cranelift** pour obtenir un vrai PIE sans `TEXTREL` (donc pouvoir retirer `-no-pie` sans régression de sécurité) — défense en profondeur, pas un bug fonctionnel, déjà explicitement différé par le ticket clos faute de besoin concret. *(Dangereuse — touche l'émission d'adresses dans tout le codegen Cranelift)* → [détails](roadmap.d/securite-pie-cranelift-is-pic.md)

(Le point qui se trouvait ici sur la convention de nommage est clos — décidée dans [docs/conventions.md](conventions.md) et portée par `ocaracs` (règles R07/R08/R09/R12), voir [qualite-convention-nommage-methodes](roadmap.d/qualite-convention-nommage-methodes.md).)

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
* On crée un test unitaire si nécessaire. Que ce soit pour le code source rust et les exemples Ocara.
* On lance les tests unitaire du code source rust d'Ocara
* On lance les test de regression et les tests unitaire des exemples Ocara
* On met à jour la documentation si nécessaire.
* Si `docs/EBNF.md` a été modifié : on relit le §31 ("Grammaire EBNF complète") pour vérifier qu'il reste réellement la source unique et à jour — toute règle ajoutée/modifiée ailleurs dans le document doit s'y refléter à l'identique (mêmes noms de règles, mêmes alternatives), sans quoi le §31 dérive silencieusement de sa propre prétention à être la référence canonique (voir [coherence-documentation-ebnf-stdlib](roadmap.d/coherence-documentation-ebnf-stdlib.md)).
* On met à jour la roadmap.
* On affiche une liste simple, sans détails, des travaux effectués afin de préparer le commit.

Si, durant les travaux, nous constatons des bugs ou d’autres points à traiter, nous évaluons s’il est possible de les intégrer au ticket en cours. Si ce n’est pas possible, nous ajoutons ces nouvelles tâches à la roadmap.

---

## Suivi

Ce document est mis à jour au fil des avancées : quand un point est traité, le retirer de la section correspondante et mettre à jour ou supprimer la fiche technique associée dans `docs/roadmap.d/`.
