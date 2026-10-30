# Roadmap Ocara

_Dernière mise à jour : 2026-10-30_

Ce document liste ce qu'il reste à faire pour faire d'Ocara un langage solide. Cette révision fait suite à une analyse complète du projet (doc, code source du compilateur, exemples, runtimes natifs) et **reprioritise délibérément autour de la robustesse, la stabilité et la fiabilité** — avant toute nouvelle fonctionnalité ou tout chantier de portage. Le focus reste, comme avant, la **gestion mémoire** : le compilateur n'a pas de ramasse-miettes (choix assumé et définitif), et l'historique du projet montre plusieurs SEGFAULTs confirmés par reproduction sur la représentation `mixed` — mais la même question de fiabilité se posait aussi sur le mécanisme d'exceptions (`setjmp`/`longjmp`), sur la quasi-absence de tests Rust unitaires en dehors du front-end, et sur au moins une race condition documentée (`HTTPServer`).

Ce fichier ne contient volontairement **aucun détail technique**. Chaque point renvoie vers une fiche dans [`docs/roadmap.d/`](roadmap.d/) pour l'implémentation, les fichiers concernés et les extraits de reproduction. À mettre à jour au fil des avancées : un point traité doit être retiré et sa fiche technique mise à jour ou supprimée. L'historique complet des correctifs déjà faits vit dans `git log`, pas dans ce fichier.

## Définition : « le langage est stable »

Cette roadmap est construite pour qu'on puisse dire que le langage est stable **quand la section "Priorité Haute" ci-dessous est vide** — pas avant. Ce n'est pas un objectif séparé à suivre en plus des tickets : c'est littéralement ce que cette section représente. Volontairement, aucune checklist n'est dupliquée ici (le projet a déjà payé le prix d'une source de vérité dupliquée ailleurs — voir `docs/adding-builtins.md`, la double liste `OCARA_BUILTINS`) : la liste unique à vider est celle de la section "Priorité Haute".

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
- **Types numériques de largeur explicite** — ajouter `int8`…`uint64` et `float16`…`float64` ; `int`/`float` deviennent des alias de `int64`/`float64` (représentation inchangée, aucun avertissement du compilateur — `ocaracs` propose un type plus étroit quand il prouve la plage). Dépassement de tout entier contrôlé (`int` compris) via `NumericException`/`IntegerException`/`FloatException`, élargissements sûrs implicites, conversions vérifiées via `Convert`, tests `is uint8`… et `is numeric`, boucles à variable typée (`for i:uint8 in 0 to 255`, bornes vérifiées une fois à l'entrée). Deux phases : sémantique, ABI et builtins ; puis stockage compact des champs et collections typées fixes (`array<uint8>`, `array<float32>`…). *(Massive — gains mémoire/performance à mesurer, pas à présumer)* → [détails](roadmap.d/langage-types-numeriques-largeur.md)
- **Builtins MIDI et audio** — lire, éditer et enregistrer des fichiers MIDI (rendu audio via SoundFont à étudier) ; charger, éditer et enregistrer du son en WAV, OGG, MP3. Aujourd'hui `ocara.SDL` ne fait que jouer des WAV/OGG/MP3 dans une fenêtre. *(Massive)* → [détails](roadmap.d/builtins-audio-midi.md)
- **Interface native : suite d'`ocara.UI`** — `menu:`/`contextMenu:` du `.oc.ui` et `webHandlers` validés par le compilateur mais affichés/câblés par aucun moteur ; vue web sous Windows et macOS ; redimensionnement d'une fenêtre sans bordure contenant une vue web. 2 questions de conception ouvertes (liaisons déclaratives, `ocara.Tauri` compilé pour Android). *(Légère à Structurel)* → [détails](roadmap.d/langage-builtin-ui-multiplateforme.md)
- **Expression conditionnelle `then`/`else`** — `var s:string = <condition> then <valeur> else <valeur>`, imbricable, sur plusieurs lignes, parenthésable pour combiner le résultat (`(c then a else b) + d`). Précédence la plus faible (sous `or`), associative à droite, seule la branche choisie est évaluée, type = union des branches comme `match`, raffinement de la condition comme `if`. `then` devient un mot-clé (aucun usage existant). *(Légère à Structurel — nouveau nœud traversant une vingtaine de passes, contrat de propriété des valeurs à respecter)* → [détails](roadmap.d/langage-ternaire-then-else.md)

---

## Priorité Basse

Confort ou portée future — n'affecte pas la correction du compilateur ou des binaires produits.

- **Performances : pistes d'optimisation restantes** — toutes les mesures de `make perf` et `make perf-extra` sont meilleures que Python, PHP, Perl ; restent le test du drapeau multi-thread à chaque accès d'une variable capturée et la retenue de `self`/des paramètres quand l'appelée a un effet ou un receveur de type inconnu. *(Structurel)* → [détails](roadmap.d/perf-runtime-complexite.md)
- **Performances : clé littérale sur une grande map** — au-delà de 8 paires, `m["A"]`/`m.A` hache la clé à chaque accès ; pistes : cache par site d'appel (rang vérifié par pointeur), hachage précalculé avec une graine réservée aux littéraux. Les petites maps sont déjà traitées (recherche par pointeur). *(Structurel)* → [détails](roadmap.d/perf-map-cle-litterale-grande-map.md)
- **Performances : égalités fragiles avec PHP** — `make perf` (comparaison à Python, PHP, Perl, C, C++ et Rust) passe sans marge : Ocara est à égalité avec PHP 8.3 sur `replace`, `join`, `jsonb`, `regex`, `map`, `split`, `asy` — le 2026-10-27, `map` (0,21 s contre 0,20) et `jsonb` (0,22 contre 0,21) échouaient la règle de façon répétée, au même niveau sur `HEAD` que sur la branche en cours. *(Légère à Moyenne)* → [détails](roadmap.d/perf-interpretes-plus-rapides.md)
- **Builtin Image : suite** — GIF animé (lecture et écriture des images et de leurs délais), pont avec `ocara.SDL` (texture depuis une `Image`, capture de la fenêtre), texte sur une image, disponibilité Android, formats supplémentaires (TIFF, ICO, AVIF, QOI, TGA) et WEBP avec perte. Le builtin `ocara.Image` couvre déjà création, chargement, dessin, transformations et enregistrement en PNG, JPEG, WEBP, GIF, BMP. *(Légère à Structurel)* → [détails](roadmap.d/builtins-image.md)
- **Serveur de langage : requêtes bloquantes, documents sans titre** — mono-thread sans annulation (la première construction de l'index bloque 1 à 1,8 s), documents `untitled:` sans diagnostics, renommage de fichier envoyé sans vérifier les capacités du client. *(Légère à Structurel)* → [détails](roadmap.d/tooling-ocaracs-fix-et-lsp.md)

- **Extension JetBrains équivalente à l'extension VS Code** — plugin IntelliJ Platform (IDEA, CLion, RustRover, PhpStorm…) aux fonctionnalités identiques : coloration (grammaire TextMate réutilisée, couleur dédiée de `when`), diagnostics, définition, références, survol, complétion, signature help, CodeLens, ocaracs et mise en forme, commandes compiler / lancer / dump / documentation, réglages. Client du serveur de langage (`ocara --lsp`), sans heuristiques recodées ; liste de parité commune aux deux extensions. *(Moyenne après le serveur de langage, Structurel sans lui)* → [détails](roadmap.d/tooling-jetbrains-plugin.md)

---

## Priorité Très Basse

Pas important du tout pour le moment — portage/intégration massifs, aucune urgence.

- **Vraie infrastructure CI pour MySQL** (service dans un futur pipeline CI, aucun aujourd'hui). *(Légère — pour plus tard)* → [détails](roadmap.d/qualite-couverture-tests.md)
- **Vérifier le round-trip complet "zéro `.a` → binaire fonctionnel"** en une seule commande — tentative abandonnée après plus d'une heure sans sortie visible (recompilation vendored OpenSSL/SQLite, ou blocage — indiscernable sans progression affichée). *(Légère — vérification, prévoir un budget de temps important et un moyen de surveiller la progression réelle)* → [détails](roadmap.d/packaging-build-cargo.md)
- **Support Windows pour la compilation du compilateur lui-même** — `ocara`/`ocara_runtime` cross-compilés vers `x86_64-pc-windows-gnu` et vérifiés bout en bout sous Wine (compile+lien+exécution réelle ; suite ocaraunit complète verte, `make tests-windows`). Reste : `ocara.Tauri` (WebView2 exige MSVC, toolchain différent de celui utilisé ici), `ocara.SDL` (pas encore essayé), jamais testé sur une vraie machine Windows. *(Restant : Structurel à Massif pour Tauri/MSVC, Légère pour SDL)* → [détails](roadmap.d/packaging-windows.md)
- **Étudier un vrai support Android** pour la compilation du compilateur lui-même — la compilation croisée existe ; rien n'a été revérifié sur appareil depuis le comptage de références (risques ouverts : marquage de pointeurs Scudo hors pont JNI, `System::exit`). *(Massive)* → [détails](roadmap.d/packaging-android.md)
- **Applications Android hybrides (WebView + serveur HTTP Ocara embarqué)** — fonctionne bout en bout, vérifié en conditions réelles sur un émulateur x86_64 (KVM) et sur un vrai téléphone Android (`examples/advanced/mini_project`, point d'entrée unique `main.oc` multi-plateforme). Reste : cycle de vie Android (arrêt propre du serveur natif), aucune visibilité sur les logs du runtime Ocara depuis un `.so` Android. *(Massive)* → [détails](roadmap.d/packaging-android-webview-hybrid.md)
- **Builtins natifs Android : suite** — tests automatisés sur appareil, relevés continus des capteurs, localisation, vidéo et aperçu caméra, actions de notification, sélecteur de documents, PCM brut au micro, cycle de vie des demandes de permission. Les builtins `ocara.AndroidPermissions`, `AndroidNotifications`, `AndroidSensors`, `AndroidFiles`, `AndroidCamera`, `AndroidMicrophone` et `AndroidAudio` existent déjà, un crate par domaine, vérifiés sur téléphone. *(Légère à Structurel)* → [détails](roadmap.d/packaging-android-native-features.md)
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
