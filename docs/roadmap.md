# Roadmap Ocara

_Dernière mise à jour : 2026-10-21_

Ce document liste ce qu'il reste à faire pour faire d'Ocara un langage solide. Cette révision fait suite à une analyse complète du projet (doc, code source du compilateur, exemples, runtimes natifs) et **reprioritise délibérément autour de la robustesse, la stabilité et la fiabilité** — avant toute nouvelle fonctionnalité ou tout chantier de portage. Le focus reste, comme avant, la **gestion mémoire** : le compilateur n'a pas de ramasse-miettes (choix assumé et définitif), et l'historique du projet montre plusieurs SEGFAULTs confirmés par reproduction sur la représentation `mixed` — mais la même question de fiabilité se posait aussi sur le mécanisme d'exceptions (`setjmp`/`longjmp`), sur la quasi-absence de tests Rust unitaires en dehors du front-end, et sur au moins une race condition documentée (`HTTPServer`).

Ce fichier ne contient volontairement **aucun détail technique**. Chaque point renvoie vers une fiche dans [`docs/roadmap.d/`](roadmap.d/) pour l'implémentation, les fichiers concernés et les extraits de reproduction. À mettre à jour au fil des avancées : un point traité doit être retiré et sa fiche technique mise à jour ou supprimée. L'historique complet des correctifs déjà faits vit dans `git log`, pas dans ce fichier.

## Définition : « le langage est stable »

Cette roadmap est construite pour qu'on puisse dire que le langage est stable **quand la section "Priorité Haute" ci-dessous est vide** — pas avant. Ce n'est pas un objectif séparé à suivre en plus des tickets : c'est littéralement ce que cette section représente. Volontairement, aucune checklist n'est dupliquée ici (le projet a déjà payé le prix d'une source de vérité dupliquée ailleurs — voir `docs/adding-builtins.md`, la double liste `OCARA_BUILTINS`) : la liste unique à vider est celle de la section "Priorité Haute".

**La section "Priorité Haute" n'est pas vide** (audit du 2026-10-13 : bugs avérés reproduits sur le compilateur actuel) — au sens de cette définition, le langage n'est pas « stable » tant qu'elle ne l'est pas. Les sections Moyenne/Basse/Très Basse ci-dessous restent à traiter mais ne bloquent pas cette définition — voir la Légende.

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

---

## Priorité Moyenne

À traiter mais non bloquant pour la stabilité du langage.

- **Extension VS Code : nouvelles fonctions de benchmark et de profilage** — courbe de complexité (`__N__`, pente log-log), min/médiane et charge machine, comparaison A/B, compteurs du runtime (retain/release, collectes, threads), profil cliquable et CodeLens, flamegraph (perf ou échantillonnage gdb), `strace -c`, plateau de serveur, ocaraunit `--leak-check`/`--profile`, export de l'historique. *(Légère à Structurel)* → [détails](roadmap.d/tooling-vscode-benchmark-profilage.md)

- **Enums enrichis** — syntaxe `case`, backing type typé déclaré (`enum Status: string { case Pending = 'pending' }`), méthodes d'instance avec `self` référençant le cas courant et pattern `self::Case`/motifs multiples par bras dans un `match`. Vérifié : l'enum actuel est un pur groupe de constantes `int` (`EnumVariant.value: Option<i64>` câblé en dur, pas de méthodes, pas d'instanciation), et `match` ne supporte ni motif « cas d'enum » ni plusieurs motifs séparés par une virgule dans un même bras. *(Massive — nouveau système d'enum quasi complet + extension du pattern matching, plusieurs points à trancher avant d'implémenter)* → [détails](roadmap.d/langage-enum-cases-backing-methods.md)
- **Réflexion : types numériques de largeur explicite** — `int8`…`uint64` et `float16`…`float64`, avec `int`/`float` comme alias envisagés de `int64`/`float64`. Non tranché : règles de dépassement, de conversion et d'affectation, précision et valeurs spéciales des flottants, compatibilité (signatures, collections, `mixed`, builtins), représentation réelle — tout occupe aujourd'hui un mot de 64 bits, les gains mémoire ou de performance restent une hypothèse à vérifier. *(Massive si la famille complète est retenue — à réévaluer selon la piste choisie)* → [détails](roadmap.d/langage-types-numeriques-largeur.md)

---

## Priorité Basse

Confort ou portée future — n'affecte pas la correction du compilateur ou des binaires produits.

- **Performances : pistes d'optimisation restantes** — toutes les mesures de `make perf` et `make perf-extra` sont meilleures que Python ; restent les locales en pile des générateurs et des captures (`nameless`, `try`) et la retenue de `self` quand la méthode appelée peut le libérer (~15 ns). *(Structurel)* → [détails](roadmap.d/perf-runtime-complexite.md)
- **Serveur de langage : requêtes bloquantes, documents sans titre** — mono-thread sans annulation (la première construction de l'index bloque 1 à 1,8 s), documents `untitled:` sans diagnostics, renommage de fichier envoyé sans vérifier les capacités du client. *(Légère à Structurel)* → [détails](roadmap.d/tooling-ocaracs-fix-et-lsp.md)
- **ocaraunit : limites restantes** — une ligne « PASS … » imprimée par le code testé compte comme une assertion ; découverte lexicale (gabarits imbriqués) ; pas de parallélisme. *(Légère)* → [détails](roadmap.d/tooling-ocaraunit-fiabilite.md)
- **Littéral `INT_MIN` et affichage des `float` entiers** — `-9223372036854775808` refusé (« integer too large ») ; `42.0` s'affiche `42`. *(Simple)* → [détails](roadmap.d/stdlib-valeurs-et-documentation.md)

- **Extension JetBrains équivalente à l'extension VS Code** — plugin IntelliJ Platform (IDEA, CLion, RustRover, PhpStorm…) aux fonctionnalités identiques : coloration (grammaire TextMate réutilisée, couleur dédiée de `when`), diagnostics, définition, références, survol, complétion, signature help, CodeLens, ocaracs et mise en forme, commandes compiler / lancer / dump / documentation, réglages. Client du serveur de langage (`ocara --lsp`), sans heuristiques recodées ; liste de parité commune aux deux extensions. *(Moyenne après le serveur de langage, Structurel sans lui)* → [détails](roadmap.d/tooling-jetbrains-plugin.md)

---

## Priorité Très Basse

Pas important du tout pour le moment — portage/intégration massifs, aucune urgence.

- **Vraie infrastructure CI pour MySQL** (service dans un futur pipeline CI, aucun aujourd'hui). *(Légère — pour plus tard)* → [détails](roadmap.d/qualite-couverture-tests.md)
- **Finaliser l'intégration Tauri** (aujourd'hui simulation en mémoire pour `listen`/`emit`/`dialog`/`notify`). *(Massive)* → [détails](roadmap.d/builtins-tauri.md)
- **Vérifier le round-trip complet "zéro `.a` → binaire fonctionnel"** en une seule commande — tentative abandonnée après plus d'une heure sans sortie visible (recompilation vendored OpenSSL/SQLite, ou blocage — indiscernable sans progression affichée). *(Légère — vérification, prévoir un budget de temps important et un moyen de surveiller la progression réelle)* → [détails](roadmap.d/packaging-build-cargo.md)
- **Support Windows pour la compilation du compilateur lui-même** — `ocara`/`ocara_runtime` cross-compilés vers `x86_64-pc-windows-gnu` et vérifiés bout en bout sous Wine (compile+lien+exécution réelle : mutex/threads, sockets/HTTP). Reste : `ocara.Tauri` (WebView2 exige MSVC, toolchain différent de celui utilisé ici), `ocara.SDL` (pas encore essayé), jamais testé sur une vraie machine Windows, et le runtime Windows n'a pas été revérifié depuis le comptage de références. *(Restant : Structurel à Massif pour Tauri/MSVC, Légère pour SDL)* → [détails](roadmap.d/packaging-windows.md)
- **Étudier un vrai support Android** pour la compilation du compilateur lui-même — la compilation croisée existe ; rien n'a été revérifié sur appareil depuis le comptage de références (risques ouverts : marquage de pointeurs Scudo hors pont JNI, `System::exit`). *(Massive)* → [détails](roadmap.d/packaging-android.md)
- **Applications Android hybrides (WebView + serveur HTTP Ocara embarqué)** — fonctionne bout en bout, vérifié en conditions réelles sur un émulateur x86_64 (KVM) et sur un vrai téléphone Android (`examples/advanced/mini_project`, point d'entrée unique `main.oc` multi-plateforme). Reste : cycle de vie Android (arrêt propre du serveur natif), aucune visibilité sur les logs du runtime Ocara depuis un `.so` Android, généralisation en builtin `ocara.UIHybrid` (voir plus bas). *(Massive)* → [détails](roadmap.d/packaging-android-webview-hybrid.md)
- **GUI native Android pilotée depuis Ocara** — au-delà du hybride WebView, piste non tranchée (`ANativeActivity`/SDL, ou pont JNI vers les Views Android/Compose). "On verra" une fois le point précédent traité. *(Complexité non évaluée — probablement Massive)* → [détails](roadmap.d/packaging-android-gui-native.md)
- **Fonctionnalités natives Android hors GUI** (notifications, capteurs, fichiers, caméra, micro, audio, permissions) — un crate Rust séparé par domaine, pour la lisibilité, sur le même principe que `runtime_tauri`/`runtime_sdl`. *(Massive)* → [détails](roadmap.d/packaging-android-native-features.md)
- **Builtin `ocara.UI`** (GUI native unifiée : futur support GUI complet de Tauri sur desktop ↔ GUI native retenue sur Android, selon la cible de compilation) — spéculatif, dépend d'un backend GUI Android qui n'existe pas encore, et d'une clarification sur ce que "GUI complet" signifie pour Tauri (Tauri reste architecturalement une WebView, voir le ticket). *(Complexité non évaluée — probablement Massive)* → [détails](roadmap.d/langage-builtin-ui-multiplateforme.md)
- **Builtin `ocara.UIHybrid`** (WebView unifiée : Tauri+WebKit desktop ↔ WebView Android, selon la cible de compilation) — même mécanisme que `ocara.UI` ci-dessus mais pour le paradigme WebView, point de départ plus solide (les deux backends se ressemblent déjà ; le prérequis [hybride WebView Android](roadmap.d/packaging-android-webview-hybrid.md) est fait). *(Massive)* → [détails](roadmap.d/langage-builtin-ui-hybride.md)
- **Tauri : gestion du menu clic-droit et de l'inspecteur WebKit** — aujourd'hui non contrôlés par `ocara.Tauri` (menu par défaut du navigateur, inspecteur non exposé). `devtools` : simple activation de feature Cargo ; menu contextuel sur Linux/WebKitGTK : `wry` n'expose rien de portable, nécessiterait du FFI direct. *(Légère pour l'inspecteur, non évaluée pour le menu)* → [détails](roadmap.d/builtins-tauri-webkit-menu-inspecteur.md)
- **Suite différée de `securite-lien-no-pie` : activer `is_pic` côté Cranelift** pour obtenir un vrai PIE sans `TEXTREL` (donc pouvoir retirer `-no-pie` sans régression de sécurité) — défense en profondeur, pas un bug fonctionnel, déjà explicitement différé par le ticket clos faute de besoin concret. *(Dangereuse — touche l'émission d'adresses dans tout le codegen Cranelift)* → [détails](roadmap.d/securite-pie-cranelift-is-pic.md)
- **Grammaire Tree-sitter `tree-sitter-ocara/`** — `grammar.js`, `queries/highlights.scm` et corpus de tests dérivés de l'EBNF, pour colorer le code Ocara (fichiers `.oc` et blocs ```` ```ocara ```` des Markdown). Couvre directement Neovim, Helix, Zed, Emacs ; GitHub (Linguist, grammaires TextMate), GitLab (Rouge) et VS Code (TextMate) ne lisent pas Tree-sitter : pour eux, ajout d'Ocara à Linguist (seuil d'usage public) et lexer Rouge en amont, `.gitattributes` en attendant. *(Moyenne — grammaire et requêtes ; dépôt en amont hors de notre contrôle)* → [détails](roadmap.d/tooling-tree-sitter.md)
- **`consumed<N>` (usages multiples)** — idée non tranchée. *(Légère à Structurel)* → [détails](roadmap.d/reflexion-consumed-usages-multiples.md)
- **Sémantique de valeur des `struct`** — explicitement différée (les `struct` sont aujourd'hui des classes déclaratives). *(Structurel)* → [détails](roadmap.d/langage-struct-value-type.md)

---

## Méthode de travail

* On analyse la roadmap et les fichiers `roadmap.d/` associés à un ticket en cours.
* On analyse les documentations
    * Workflow, l'EBNF et les documentations lié à notre ticket
* On utilise le Makefile pour lancer la compilation, tests et les regressions
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
* Si, durant les travaux, nous constatons des bugs ou d'autres points à traiter, nous évaluons s'il est possible de les intégrer au ticket en cours. Si ce n'est pas possible, nous ajoutons ces nouvelles tâches à la roadmap.
* Si on trouve un bug on doit l'ajouter en priorité haute de la roadmap. Suivant la criticité du bug on le positionne en haut de liste.
* On ne garde pas de trace de ce qui a était traité dans la roadmap, le referentiel de git nous suffit pour tracer les taches effectué. Je veux un fichier roadmap qui se concentre uniquement sur ce qui reste à faire.

---

## Suivi

Ce document est mis à jour au fil des avancées : quand un point est traité, le retirer de la section correspondante et mettre à jour ou supprimer la fiche technique associée dans `docs/roadmap.d/`.
