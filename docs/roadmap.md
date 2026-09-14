# Roadmap Ocara

_Dernière mise à jour : 2026-09-14_

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
- ✅ **CI pour `examples/advanced/httpserver`** — nouveau script `httpserver.sh` (démarrage en fond, requêtes sur toutes les routes + cas 404, arrêt), câblé dans `ci/regression.sh`. `mini_project`/`tauri_httpserver` restent hors CI par choix explicite (les deux ouvrent une vraie fenêtre Tauri/WebView, pas seulement `tauri_httpserver` comme on le pensait initialement).
- ✅ **`builtins/mysql` ne fait plus échouer la CI locale** — `ci/regression.sh` sonde `127.0.0.1:3306` et annonce `SKIP` (pas `FAIL`) si aucun serveur MySQL/MariaDB n'est joignable, plutôt que d'échouer systématiquement faute d'infrastructure. Exemple de service CI (GitHub Actions) documenté pour le jour où ce projet aura un pipeline versionné — aucun Docker/serveur local disponible pour aller plus loin ici.
- ✅ **Syntaxe obsolète (`T[]`, `==`, `<`/`>`) nettoyée** — tous les exemples cassés compilent à nouveau (`07_loopsTest`, `08_arraysTest`, `16_typesTest`, `19_break_continueTest`, `examples/generics/*`, `examples/runtime/variables.oc`), contradiction sur les namespaces imbriqués corrigée dans l'EBNF, 20 fichiers `.bak` obsolètes supprimés. `make regression` termine désormais en succès complet (`exit 0`).
- ✅ **`Map::forEach` implémenté** — le callback (fat pointer, même mécanisme que `HTTPServer::route`) est maintenant réellement appelé pour chaque entrée ; le stub mort `__map_foreach` (jamais invoqué par aucun chemin de lowering, malgré ce que laissait penser un commentaire de doc) est retiré. Découverte annexe non corrigée : l'arithmétique entre `int` et une valeur `mixed` produit un résultat faux (nouvelle fiche).
- ✅ **Lien OpenSSL inconditionnel retiré du build du compilateur** — `build.rs` liait `-lssl`/`-lcrypto` au binaire `ocara` lui-même, en contradiction directe avec le choix documenté de `src/codegen/link.rs` (OpenSSL vendored, aucun lien dynamique nécessaire nulle part). Rebuild complet du compilateur + compilation/exécution d'un exemple MySQL confirment que ce n'était pas nécessaire.
- ✅ **6 bugs mémoire distincts corrigés autour de `scoped`/`consumed`, chacun reproduit puis vérifié** : fuite selon le chemin d'exécution (branches d'un if/switch), double-free d'une `consumed` réutilisée en boucle, fuite à la réaffectation, SEGFAULT sur double fermeture manuelle+automatique (Mutex/SQLite confirmés), abort sur double `.join()`/`.detach()` d'une `Thread` (nouveau diagnostic **E22**), et — découverte bien plus large que prévu — un **use-after-free général du shadowing** touchant n'importe quelle variable (pas seulement `scoped`/`consumed`) : un nom réutilisé dans un bloc imbriqué corrompait définitivement la résolution de la variable externe. `Stmt::Result` émet aussi maintenant les mêmes destructions anticipées que `Stmt::Return`.
- ✅ **`scoped`/`consumed` déclarée directement dans un bloc `main`/`init`/`exit` n'était jamais libérée** — `lower_runtime_main_manual` lowered ces statements à plat (`lower_stmt` direct) sans jamais passer par `lower_block`, court-circuitant tout le mécanisme de libération de fin de bloc (`error`/`success` n'étaient pas concernés : déjà de vrais `Block` lowered via `lower_if`). Corrigé en enveloppant `init`+`main` et `exit` dans un vrai `Block`, lowered via `lower_block` comme n'importe quel bloc normal — `ERROR`/`SUCCESS` (jamais `scoped`/`consumed`) restent seules lowered hors bloc pour survivre à toutes les phases suivantes.
- ✅ **Synchronisation réelle des variables capturées par une closure/thread** — chaque capture "promue sur le tas" (partagée entre le scope extérieur et une closure, potentiellement sur des threads différents) est désormais protégée par un vrai mutex (`__alloc_locked_cell`/`__locked_cell_get`/`__locked_cell_set`), au lieu d'un accès brut non défini. Limite assumée et choisie explicitement : les opérations composées (`x = x + 1` entre threads) restent sujettes aux pertes de mise à jour classiques (pas d'atomicité, comme un `int` C ordinaire) — seule l'absence de synchronisation (UB, aliasing Rust) est corrigée.
- ✅ **Typage des accès membres sur une valeur générique** — `numbers.add("texte")` sur un `List<int>` est maintenant rejeté (substitution réelle des paramètres de type par les arguments concrets de l'instance, avant de vérifier arité/types d'arguments/type de retour), au lieu de retomber silencieusement sur `Type::Mixed`.
- ✅ **Un générique fonctionne maintenant en paramètre de fonction/méthode et en champ de classe** (`var_class` et la résolution de champ chaîné ne reconnaissaient que les variables locales directement initialisées) — confirmé par reproduction (`useBox(b:Box<int>)`/`self.box.get()` renvoyaient `0` au lieu de la vraie valeur), corrigé et vérifié y compris pour un générique auto-référent façon liste chaînée (`Node<T> { property next:Node<T> }`) et pour la libération/le clonage récursifs d'un champ générique (fuite + aliasing corrigés). Découverte annexe du tour précédent (`self.value = v` produisant `1` au lieu de `100`) retestée : ne se reproduit plus, vraisemblablement déjà corrigée par le correctif shadowing général de la même session.
- ✅ **`implements` sur un `generic` est maintenant vérifié** (E09) — un `generic Bag<T> implements Sized` sans la méthode `size()` compilait sans erreur, quel que soit le nombre d'instanciations ; la vérification couvre maintenant aussi `program.generics`, pas seulement les classes concrètes.
- ✅ **Arithmétique `+`/`-`/`*`/`/` entre un type connu et une valeur `mixed`** — `y + x` avec `x:mixed` contenant un entier traitait systématiquement l'opération comme une concaténation string (`105` devenait `1062449896`, le pointeur de `"1095"`). Nouveau dispatch runtime dynamique (`__dyn_add`/`__dyn_sub`/`__dyn_mul`/`__dyn_div`, même principe que les comparaisons strictes) qui décide réellement, au runtime, entre concaténation et calcul numérique (entier ou flottant). Découverte annexe corrigée en cours de route, plus large que prévu : `box_for_any` ne savait jamais déballer un `mixed` vers une cible concrète (`var f:float = mixedValue` était déjà faux, indépendamment de toute arithmétique), et un `mixed` contenant un float combiné à un `int` connu (jamais `float`) dans `-`/`*`/`/` était silencieusement tronqué en entier — les deux sont corrigés.
- ✅ **Les littéraux `float`/`bool` dans un `array`/`map` ne sont plus stockés comme des strings** — tout élément `float`/`bool` d'un littéral était systématiquement stringifié, **y compris hors `mixed`** : un `array<float>` littéral (pas seulement `array<mixed>`) était donc entièrement cassé à la lecture (bits d'un pointeur de string réinterprétés comme un float). Corrigé : brut pour un type d'élément concret connu (`array<float>`), boxé (`__box_float`/`__box_bool`) pour `mixed`/type inconnu à cet endroit — `JSON::encode`/`YAML::encode` mis à jour pour déballer correctement (`value_to_json` ne vérifiait même pas les floats boxés). Découvertes annexes non corrigées, distinctes : le passage d'un `float`/`bool` en argument `mixed` ne le boxe toujours pas, et `IO::writeln(JSON::encode(x))` sans variable intermédiaire affiche un nombre incohérent (dispatch de type, sans rapport avec le stockage).
- ✅ **Diagnostic de double-free explicite sur une ressource (E25)** — `m.destroy(); m.destroy()` (deux appels manuels dans le code, pas la libération automatique de fin de bloc) provoquait un SEGFAULT confirmé, sans le moindre diagnostic. Généralisation du mécanisme déjà en place pour `Thread.join()`/`.detach()` (E22) à `Mutex::destroy`/`SQLite::close`/`MySQL::close`/`MariaDB::close`. Les deux autres diagnostics manquants identifiés (fuite d'un handle natif déclaré en `var`, fuite d'un champ de classe non pris en charge) restent bloqués : ils nécessitent la stratégie mémoire pour `var` (voir Priorité Haute ci-dessous, Massive).
- ✅ **`on e is X` valide maintenant le nom de la classe (E23) et impose le catch-all en dernier (E24)** — un typo dans le nom de classe filtrée compilait silencieusement (handler mort, jamais atteint), et rien n'imposait la règle documentée "le catch-all doit être en dernier". Découverte annexe : `SDLException`/`TauriException`, réellement levées par le runtime, n'étaient enregistrées nulle part côté sema — ajoutées, et toutes les classes d'exception builtin sont désormais "ambiantes" (reconnues sans import explicite, comme c'est déjà l'usage réel dans le dépôt). La hiérarchie d'exceptions réelle (`extends`), plus structurelle, reste ouverte (voir ci-dessous).
- ✅ **Second chemin de chargement des imports (ancien format), redondant et incomplet, supprimé** — masquait les interfaces/modules d'un fichier importé dès que le symbole demandé n'était pas une classe (`import main` résolvant à la fonction `main` du fichier, sans jamais fusionner les interfaces `implements`ées par les classes du même fichier). Lacune annexe comblée en le supprimant : un import sélectif ne rapatriait jamais les consts de premier niveau du fichier source (seul `import *` le faisait). `examples/project/tests/mainTest.oc`, qui s'appuyait sur ce bug, corrigé pour utiliser la syntaxe d'import documentée — **`make regression` se termine pour la première fois sans une seule erreur de compilation** (49 PASS / 0 FAIL / 0 ERREUR côté tests projet, contre 22 PASS / 1 ERREUR).
- ✅ **`cargo build -p ocara` fonctionne maintenant seul, sans `make build` au préalable** — `build.rs` compile lui-même à la volée chaque `.a` runtime manquant (`cargo build --release -p <crate>` imbriqué), réplique aussi la logique du shim pkg-config GTK/WebKit pour `ocara_runtime_tauri`. Vérifié : `make build`/`make regression` inchangés (chemin déjà-construit), et le déclenchement de chaque recompilation imbriquée confirmé individuellement (aucun signe de blocage). Un aller-retour minuté complet "zéro `.a` → binaire" n'a pas pu être mené à son terme dans cette session (chaque dépendance C vendored du runtime — OpenSSL, SQLite — prend plusieurs dizaines de secondes à se recompiler depuis les sources dès qu'un cache est invalidé) ; voir la fiche pour le détail de ce qui a pu, et n'a pas pu, être vérifié.

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
- **Concevoir une vraie stratégie de gestion mémoire pour `var`** (et pour les environnements de closures) — le mot-clé par défaut du langage ne libère aujourd'hui jamais rien. *(Massive)* → [détails](roadmap.d/memoire-strategie-var.md)

---

## Priorité Moyenne

### Gestion mémoire

- **Robustifier les libérations bas niveau du runtime** (tag d'exception ambigu, taille recalculée sans header fiable, détection de type par heuristique sur un entier). *(Dangereuse)* → [détails](roadmap.d/memoire-fiabilite-runtime-bas-niveau.md)

### Langage

- **Donner une existence réelle aux interfaces à l'exécution** (dispatch dynamique, aujourd'hui purement statique). *(Massive)* → [détails](roadmap.d/langage-interfaces.md)
- **Faire marcher une vraie hiérarchie d'exceptions** (`__ocara_type_matches` reste une égalité de chaînes stricte, sans parcours de `extends` — une sous-classe n'est pas attrapée par un filtre sur sa classe parente). *(Structurel)* → [détails](roadmap.d/langage-exceptions.md)

---

## Priorité Basse

### Builtins

- **Finaliser l'intégration Tauri** (aujourd'hui simulation en mémoire pour `listen`/`emit`/`dialog`/`notify`). *(Massive)* → [détails](roadmap.d/builtins-tauri.md)

### Build & portabilité

- **Étudier un vrai support Windows** pour la compilation du compilateur lui-même. *(Massive)* → [détails](roadmap.d/packaging-windows.md)
- **Étudier un vrai support Android** pour la compilation du compilateur lui-même. *(Massive)*

---

## Suivi

Ce document est mis à jour au fil des avancées : quand un point est traité, le retirer de la section correspondante et mettre à jour ou supprimer la fiche technique associée dans `docs/roadmap.d/`.
