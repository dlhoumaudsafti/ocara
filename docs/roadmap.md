# Roadmap Ocara

_Dernière mise à jour : 2026-10-18_

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

- **Map capturée par une fermeture, indexée : SIGSEGV** — `m["a"] = 5` dans une `nameless` puis `m["a"]` → SIGSEGV (métadonnées de capture non propagées) ; casse le partage d'une map entre threads et handlers ; fonction capturée rendant une `string` → fuite par appel. *(Simple)* → [détails](roadmap.d/langage-closure-metadonnees-capture.md)
- **`async`/`resolve` : double free, handles boxés, appels synchrones** — Second `resolve` → double free ; `hs.push(sq(3))` puis `resolve` → SIGSEGV ; `async` dans une `nameless`/générateur appelé en synchrone → SIGSEGV ; tâche jamais résolue → ~36 Ko perdus par appel. *(Structurel)* → [détails](roadmap.d/async-resolvable-handles.md)
- **Builtins `int`/`bool` sans type de retour : SIGSEGV** — `IO::writeln(File::size(f))` → SIGSEGV ; `${res.status()}`, `${Thread::currentId()}` → `null` ; `${r.body()}` fuit (table `fn_ret_types` incomplète). *(Légère)* → [détails](roadmap.d/builtins-types-retour-manquants.md)
- **`Array::join`/`sort`/`contains` et affichage des conteneurs** — `Array::join([100000])` et `array<float>` → SIGSEGV ; tri et `contains` comparent les chaînes par adresse, `sort` de floats sur les bits ; `IO::writeln(array)` imprime des octets ; assertions `UnitTest` fausses sur tableaux et floats. *(Légère)* → [détails](roadmap.d/stdlib-array-elements-bruts.md)
- **Runtime : paniques Rust et blocages sur entrées limites** — `Math::clamp(5, 10, 0)` → abort ; `.env` `A="` → blocage, `=value` → abort ; `sleep` négatif ne rend jamais la main ; SQLite `rollback` bloqué à vie (verrou tenu pendant un `longjmp`). *(Légère)* → [détails](roadmap.d/runtime-paniques-et-blocages.md)
- **HTTPServer : arrêt sur exception, injection CRLF, déni de service** — Exception non rattrapée dans un handler → serveur arrêté ; `respondHeader` permet d'injecter des en-têtes ; N téléversements lents bloquent le serveur ; UTF-8 encodé → mojibake ; fichiers statiques binaires corrompus. *(Légère à Structurel)* → [détails](roadmap.d/stdlib-httpserver-robustesse.md)
- **Exception dans un `Thread` : arrêt du processus** — Promis : `ThreadException` 102 au `join()` ; obtenu : arrêt (rc 1) — `Thread_run` sans `run_closure_catching`. *(Simple)* → [détails](roadmap.d/stdlib-thread-exception.md)
- **Compilateur et programmes : débordements de pile** — Expression de 800 niveaux ou somme de 5 000 termes → abort du compilateur ; récursion profonde → SIGSEGV muet ; complexité O(n²) du parseur et O(F²) de l'abaissement, `elseif` × 10 000 → 1,6 Go. *(Légère à Structurel)* → [détails](roadmap.d/compilateur-limites-echelle.md)
- **Fuites : exceptions, méthodes JSON, HTMLComponent** — `raise` dans un `on` → l'exception `e` fuit ; `return` dans un `on` → locales perdues ; constructeur qui lève → objet perdu ; `m.encode()`/`s.decode()` fuient leur résultat ; `HTMLComponent` ne relâche jamais ses closures. *(Légère)* → [détails](roadmap.d/memoire-fuites-exceptions-et-builtins.md)
- **`and`/`or` sans court-circuit ; condition `mixed` testée comme un pointeur** — `n not equal null and n.v equal 1` lève « Cannot access 'v' on null » ; `if mixed(false)` prend la branche (maps JSON). *(Légère)* → [détails](roadmap.d/langage-and-or-court-circuit.md)
- **`try` : `break`/`continue`/`return` perdus ; locales de générateur perdues** — `continue`/`break` dans un `try` ignorés (`01234` au lieu de `02`, boucle infinie) ; `return` dans un `try` → erreur de codegen ; locales d'un générateur perdues après un `raise` rattrapé. *(Structurel)* → [détails](roadmap.d/langage-try-controle-de-flux.md)
- **Dispatch et héritage : valeurs fausses** — Appel via interface sur une sous-classe de l'implémenteur → `null` ; `parent::` dans une méthode héritée → `BBA` au lieu de `BA` ; générique instancié seulement dans un générique → champs `null` ; modules : erreurs de codegen. *(Légère à Structurel)* → [détails](roadmap.d/langage-dispatch-heritage-generiques.md)
- **Sema : programmes invalides acceptés qui plantent** — `types_compat` trop permissif (union, `null`, covariance des conteneurs et de `Function`) ; redéfinition incompatible, champ générique, branche de `match`, `-` unaire, indexation de chaîne, chemin sans `return`, déclarations en double non signalées. *(Structurel)* → [détails](roadmap.d/sema-solidite-typage.md)
- **Appels indirects rejetés par le codegen** — `adder(4)(1)`, `fs[0](5)` → « fonction inconnue : _unknown » ; méthode générateur statique → compilation cassée même non appelée ; variable `for` sur `message<Function>` non appelable. *(Légère)* → [détails](roadmap.d/langage-appels-indirects.md)
- **Bibliothèque standard : valeurs fausses et doc divergente** — `File::copy(p, p)` vide le fichier ; dates avant 1970 fausses, dates impossibles acceptées ; `String::empty("   ")`, `Math::round(-2.5)`, `pow` qui déborde ; une dizaine de docs contredisent le code. *(Légère)* → [détails](roadmap.d/stdlib-valeurs-et-documentation.md)
- **Performances : `map` et concaténation quadratiques, collecteur de cycles** — Map 10⁵ insertions → ~20 s (recherche linéaire) ; `s = s + "x"` × 10⁶ → 67 s ; collecte de cycles qui reparcourt le graphe vivant ; méthode ~26 ns (retain/release de `self`) ; deux appels système par exception. *(Structurel)* → [détails](roadmap.d/perf-runtime-complexite.md)
- **Bilan `--leak-check` : fuite du `main` runtime attribuée à la ligne 0** — le `main` synthétisé des blocs runtime n'a pas de position, le site d'allocation affiché est `fichier.oc:0`. *(Simple)* → [détails](roadmap.d/langage-runtime-blocs-variables.md)
- **ocaraunit : cache de ~110 Go, blocages, faux positifs** — Cache jamais purgé (95 Go dans `examples/`) ; test qui boucle → runner bloqué ; cache périmé pour un import hors racine ; tests ignorés sans message et exit 0. *(Légère)* → [détails](roadmap.d/tooling-ocaraunit-fiabilite.md)
- **CLI : `-o` sur le source l'écrase ; runtime laissé dans `/tmp`** — `ocara x.oc -o x.oc` remplace le source par l'exécutable ; 79 Mo extraits dans `/tmp` par compilation, laissés si interrompue ; options inconnues acceptées. *(Simple)* → [détails](roadmap.d/tooling-cli-compilateur.md)
- **ocaracs `--fix`, serveur de langage et extension : code cassé, états périmés** — `--fix` convertit CRLF dans les chaînes, fusionne `userID`/`userId` ; serveur : importeurs non réanalysés, renommage sans conflit ; extension : délai du benchmark qui ne tue pas le programme, grammaire cassée par `\``. *(Légère)* → [détails](roadmap.d/tooling-ocaracs-fix-et-lsp.md)

---

## Priorité Moyenne

À traiter mais non bloquant pour la stabilité du langage.

- **Extension VS Code : nouvelles fonctions de benchmark et de profilage** — courbe de complexité (`__N__`, pente log-log), min/médiane et charge machine, comparaison A/B, compteurs du runtime (retain/release, collectes, threads), profil cliquable et CodeLens, flamegraph (perf ou échantillonnage gdb), `strace -c`, plateau de serveur, ocaraunit `--leak-check`/`--profile`, export de l'historique. *(Légère à Structurel)* → [détails](roadmap.d/tooling-vscode-benchmark-profilage.md)

- **Enums enrichis** — syntaxe `case`, backing type typé déclaré (`enum Status: string { case Pending = 'pending' }`), méthodes d'instance avec `self` référençant le cas courant et pattern `self::Case`/motifs multiples par bras dans un `match`. Vérifié : l'enum actuel est un pur groupe de constantes `int` (`EnumVariant.value: Option<i64>` câblé en dur, pas de méthodes, pas d'instanciation), et `match` ne supporte ni motif « cas d'enum » ni plusieurs motifs séparés par une virgule dans un même bras. *(Massive — nouveau système d'enum quasi complet + extension du pattern matching, plusieurs points à trancher avant d'implémenter)* → [détails](roadmap.d/langage-enum-cases-backing-methods.md)
- **Réflexion : types numériques de largeur explicite** — `int8`…`uint64` et `float16`…`float64`, avec `int`/`float` comme alias envisagés de `int64`/`float64`. Non tranché : règles de dépassement, de conversion et d'affectation, précision et valeurs spéciales des flottants, compatibilité (signatures, collections, `mixed`, builtins), représentation réelle — tout occupe aujourd'hui un mot de 64 bits, les gains mémoire ou de performance restent une hypothèse à vérifier. *(Massive si la famille complète est retenue — à réévaluer selon la piste choisie)* → [détails](roadmap.d/langage-types-numeriques-largeur.md)

---

## Priorité Basse

Confort ou portée future — n'affecte pas la correction du compilateur ou des binaires produits.

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
