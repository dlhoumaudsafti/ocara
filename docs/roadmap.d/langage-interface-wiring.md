# Nouvelle instruction `wiring` dans une interface — liaison interface↔implémentation à la compilation

Vérifié :
- **Toutes les règles de résolution tranchées dans ce ticket sont implémentées et vérifiées bout-en-bout** (compilation + exécution réelle, pas seulement `--check`) : `wiring <chemin.pointé>` répétable dans un corps `interface` ; nom **nu** (`use Interface(...)`, `Interface::méthode()`) résolvant vers le **premier** `wiring` déclaré (ordre textuel) ; import **aliasé** (`import Interface as X`) dont `X` correspond au nom simple d'un `wiring` devenant un substitut complet de cette classe concrète PARTOUT (type, construction, appel statique) ; nom réel de l'interface en annotation de type ordinaire restant le type abstrait, polymorphisme intact ; `wiring` agissant comme import implicite de chaque classe visée.
- **Pipeline en deux passes** retenu pour résoudre un problème d'ordre réel : la file `imports_to_process` (`src/main.rs`) charge les fichiers en largeur, pas dans un ordre de dépendance — un fichier qui écrit `import Interface as X` peut être traité AVANT que le fichier déclarant `Interface` (et donc ses `wiring`) n'ait été chargé. Résolu par (1) un pré-scan tolérant aux erreurs, `core::interface_wiring::collect_all_interfaces`, qui parcourt tout le graphe d'imports transitif une fois, AVANT toute résolution d'alias (y compris celle du fichier principal), pour connaître par avance les `wiring` de CHAQUE interface atteignable ; et (2) une passe post-fusion, `core::interface_wiring::resolve_bare_interface_names`, qui réécrit `Expr::New`/`Expr::StaticCall` (jamais `Type::Named`) vers le premier `wiring`, exécutée une fois le programme entier fusionné (aucun souci d'ordre à ce stade).
- `core::alias_resolve::compute_aliases` étendu (signature à 3 arguments : `imports`, `all_interfaces`, `current_file`) pour détecter un alias correspondant au nom simple d'un `wiring` et le faire résoudre directement vers la classe concrète — `resolve_aliases` (la passe qui réécrit réellement l'AST) reste totalement inchangée : elle traitait déjà `Type::Named` PARTOUT (champs, paramètres, retours...), donc la substitution "alias → type concret en annotation de type" fonctionne automatiquement, sans logique supplémentaire.
- **Prérequis de grammaire découvert et ajouté, hors texte littéral du ticket mais nécessaire à ses propres exemples** : le corps d'une `interface` n'acceptait ni `public` ni `static` devant `method` avant ce ticket (confirmé par test direct : `error: expected Method, found Public`) — pourtant les exemples du ticket lui-même utilisent `public static method`/`public method`. Ajouté : `public` cosmétique (ignoré, toute méthode d'interface est publique par nature), `static` sémantique (`InterfaceMethod::is_static`, nécessaire pour qu'une interface exige un contrat statique — le cas d'usage central d'un `wiring` vers une fabrique statique).
- **Correction de prose relevée** : `implement` (singulier), utilisé partout dans le texte du ticket, n'est pas un mot-clé réel — seul `implements` (pluriel) existe (`src/parsing/lexer.d/tokenizer.d/keywords.rs`). Traité comme une imprécision de rédaction ; le code et les tests utilisent `implements` partout.
- **Vérification de conformité étendue à la staticité** : la vérification `implements` déjà existante (directement dans `src/main.rs`, "§4d"/"§4d-bis", PAS via `SemaError` — mécanisme déjà en place avant ce ticket, réutilisé tel quel) ne comparait jusqu'ici jamais `is_static` entre l'interface et la classe (impossible à observer avant ce ticket, `static` n'existant pas dans une interface) — un garde-fou `is_static` a été ajouté aux DEUX boucles (classe ET generic), pas seulement pour les classes visées par un `wiring` : toute interface qui exige désormais une méthode statique doit être honorée par une méthode statique, quel que soit le mécanisme (`wiring` ou `implements` simple).
- **Nouvelles vérifications de conformité propres à `wiring`** (§4d-ter, `src/main.rs`, même famille que §4d/4d-bis ci-dessus) : classe cible introuvable (ni classe ni generic — un `wiring` vers un `generic` NU est explicitement rejeté, ambigu tant qu'il n'est pas instancié) ; classe cible qui n'`implements` pas (textuellement) l'interface qui la `wiring` (sans quoi §4d ne la voit jamais, puisqu'elle n'itère que `class_decl.implements`) ; deux `wiring` de la même interface partageant le même nom simple (signalé sur l'interface elle-même, les deux `WiringDecl` fautifs référencés dans le message).
- **Bug de codegen réel découvert et corrigé, atteignable UNIQUEMENT depuis l'extension de grammaire ci-dessus** (`static` dans une interface était auparavant impossible à écrire) : `src/lower/builder.d/interfaces.rs::generate_interface_dispatchers` générait TOUJOURS un dispatcher `self`-based pour chaque méthode d'interface implémentée par au moins une classe, y compris les méthodes STATIQUES — une méthode statique n'a pourtant aucun `self` sur lequel dispatcher à l'exécution (sa cible est déjà résolue À LA COMPILATION, par `wiring`/l'alias). Le dispatcher mort (`Interface_méthode(self)` appelant `Classe_méthode(self)`, alors que la vraie méthode statique — sans paramètre `self` — n'attend aucun argument) faisait échouer la vérification Cranelift du MODULE ENTIER, même quand ce dispatcher n'était jamais appelé par le moindre code utilisateur — confirmé par reproduction minimale avant correctif (`mismatched argument count for ...: got 1, expected 0`), y compris sans le moindre `wiring` (dès qu'une interface déclare `static method X()` et qu'une classe l'implémente). Corrigé en sautant purement et simplement les méthodes `is_static` dans `generate_interface_dispatchers` : un appel statique n'a jamais besoin d'un dispatcher runtime. Couvert par 2 nouveaux tests Rust (`src/lower/builder.d/interfaces.rs`) et par `examples/tests/60_interface_wiringTest.oc` (tout appel statique interface y passe).
- **Bug d'implicite-import découvert et corrigé pendant l'implémentation** : `wiring <Classe>` colocalisée dans le MÊME fichier que l'interface (le cas le plus simple, celui des exemples illustratifs) déclenchait systématiquement une tentative de CHARGER UN FICHIER SÉPARÉ du même nom (`error: reading file '.../PostgresRepo.oc': No such file or directory`), puisque l'import implicite ne savait pas que la classe visée était déjà présente dans le fichier en cours de traitement. Corrigé : `enqueue_wiring_imports` (`src/main.rs`) vérifie d'abord si la cible est déjà connue (`program.classes`) ou colocalisée dans le fichier qu'on vient de charger (`mod_prog.classes`, passé en `local_pool`) — dans ce dernier cas, la classe est directement RAPATRIÉE (clonée) dans `program.classes`, sans le moindre import de fichier. Un doublon temporaire et inoffensif est possible dans le cas `import *` (nettoyé par le dédoublonnage §4b existant, plus loin dans `main()`) ; les 3 autres sites d'appel n'en produisent aucun.
- **Diagnostics** (`docs/diagnostics.md`) : E38 (construction/appel statique sur une interface sans `wiring`, `SemaError::InterfaceNoWiring`, `src/sema/typecheck.rs`), E39 (cible `wiring` introuvable), E40 (cible `wiring` n'`implements` pas l'interface), E41 (alias ne correspondant à aucun `wiring`, `core::alias_resolve`), E42 (deux `wiring` de même nom simple) — E39/E40/E42 vivent dans `src/main.rs` (§4d-ter), même famille/mécanisme que E09 (`diagnostic::print_error` + `exit(1)`, pas `SemaError`), E41 dans `core::alias_resolve::compute_aliases`. `wiring` hors d'un corps `interface` n'a nécessité AUCUN code dédié : rejeté nativement par le parser (`unexpected top-level declaration: Wiring`), documenté dans la table des erreurs syntaxiques.
- **Bug préexistant, SANS RAPPORT avec ce ticket, découvert pendant les tests et délibérément NON corrigé ici** : assigner une instance concrète à une variable TYPÉE PAR L'INTERFACE puis appeler une méthode dessus (`var s:Shape = c; s.area()`, où `Shape` est une interface et `c` une instance concrète) provoque un SIGSEGV — confirmé par reproduction directe. Cause : le dispatch dynamique par identité de classe (`class_dispatcher_name`, `src/lower/builder.d/class_dispatch.rs`) ne connaît que l'héritage de CLASSE (`classes_with_subclasses`), jamais les interfaces — malgré l'existence de `generate_interface_dispatchers` (`src/lower/builder.d/interfaces.rs`, mécanisme réel mais distinct, voir sa propre doc de module), qui ne semble pas branché sur ce chemin d'appel précis pour un appel d'INSTANCE via une variable interface-typée. Confirmé ORTHOGONAL à `wiring` : la substitution de `wiring` produit toujours des bindings CONCRETS (jamais interface-typés) dès qu'elle s'applique, donc ne traverse jamais ce chemin cassé — mais une variable interface-typée ORDINAIRE (polymorphisme classique, sans aucun `wiring`) reste vulnérable, ticket ou pas. Recommandation : ouvrir un ticket dédié, distinct, pour ce bug (dispatch d'instance sur variable interface-typée) — hors périmètre ici.
- **Tests Rust** : 22 nouveaux dans `src/` — parser (`src/parsing/parser.d/tests.rs` : `wiring` simple/multiple/ordre préservé/interleavé avec les méthodes/absence, modificateurs `public`/`static` cosmétique vs sémantique), `core::alias_resolve`/`core::tests` (alias → premier/second `wiring`, interface sans `wiring` → cosmétique, symbole non-interface inchangé — 3 tests existants adaptés à la nouvelle signature à 3 arguments), `core::interface_wiring` (module de tests inline : réécriture nom nu → premier `wiring` pour `use`/appel statique, interface sans `wiring` intacte, `Type::Named` jamais touché, récursion dans une `const` de classe, no-op total sans aucun `wiring`), `src/sema/tests/interface_wiring.rs` (E38 construction/appel statique, non-régression construction avec `wiring`, type en annotation jamais signalé, staticité bien enregistrée en table des symboles), `src/lower/builder.d/interfaces.rs` (2 tests : aucun dispatcher pour une méthode statique, dispatcher toujours généré pour une méthode d'instance). `cargo test --bin ocara --release` : **164 passed, 0 failed** (142 avant ce ticket + 22). `cargo test -p ocara_runtime --release` : 105 passed, 7 ignored (inchangé, aucun fichier runtime touché).
- **Exemple de régression bout-en-bout** : `examples/60_interface_wiring.oc` (illustratif, exécutable seul — interface `Repo`, deux `wiring`, polymorphisme via le type abstrait) et `examples/tests/60_interface_wiringTest.oc` (6 assertions : nom nu → premier wiring en construction ET en appel statique, alias → second wiring en construction ET en appel statique, type-annotation reste abstrait et accepte les deux implémentations concrètes). `examples/advanced/mini_project`/`mini_project_hexa` explicitement PAS touchés dans ce chantier (contrainte du ticket).
- `make build` (les 4 crates) + `RUSTFLAGS="-D warnings"` : 0 warning. `./ci/regression.sh` : tous verts. `./ci/unittests.sh examples/project/tests` : 50 PASS / 0 FAIL (inchangé). `./ci/unittests.sh examples/tests` : **778 PASS / 0 FAIL, 0 ERREUR(S)** (772 avant ce ticket, +6 nouvelles assertions dans le nouveau fichier 60).
- `docs/EBNF.md` : nouveau mot-clé `wiring` dans la liste des mots réservés (§8), §17 étendue (`InterfaceMember`, `InterfaceMethod` avec `public?`/`static?`, `WiringDecl`) avec une nouvelle sous-section 17.1 dédiée (règles de résolution + exemple), grammaire consolidée §31 mise à jour en miroir et relue intégralement après coup (convention du projet). `tools/highlight/vsode/syntaxes/ocara.tmLanguage.json` : `wiring` ajouté à `keyword-other`, nouvelle règle `wiring-declaration` colorant le nom de classe visé comme une classe (même famille que `class-instantiation`) — `resolver.ts`/`completion.ts` non touchés (`wiring` ne déclare aucun nouveau symbole autonome, rien à ajouter à leurs regex de déclaration `generic|class|interface`).

## Proposition (ticket original)

Permettre à une `interface` de déclarer, en son sein, une ou plusieurs
classes concrètes qui la servent — pour que le code consommateur ne dépende
JAMAIS d'une implémentation concrète (infra), seulement du contrat (domain).
Objectif explicite : renforcer le respect de l'architecture hexagonale déjà
en place dans `mini_project_hexa` (voir
[stdlib-httpserver-request-object](stdlib-httpserver-request-object.md) et
les tickets HTTPServer précédents, qui ont servi de terrain à cette
architecture) — le service applicatif ne doit importer que le port, jamais
l'adaptateur.

**Mécanisme unifié (généralisé après discussion — ne se limite pas aux
méthodes statiques)** : importer une interface avec un alias qui correspond
au nom simple d'un de ses `wiring` fait de cet alias un **stand-in
transparent pour la classe concrète visée**, utilisable PARTOUT où on
écrirait normalement le nom de cette classe — annotation de type,
construction (`use X(...)`), appel statique (`X::méthode()`) — après quoi
les appels d'instance (`x.méthode()`) suivent normalement, puisque `x` est
alors typé comme la classe concrète elle-même.

```ocara
// context/home/domain/contract/CarSummaryInterface.oc
interface CarSummaryInterface {
    wiring context.home.infra.db.CarSummaryRepository
    wiring context.home.infra.db.SuperCarSummaryRepository
    public static method all(): array<CarSummaryEntity>
}
```

Trois façons d'appeler, selon l'import dans le service consommateur :

```ocara
import context.home.domain.contract.CarSummaryInterface
CarSummaryInterface::all() // pas d'alias → 1er wiring déclaré
```
```ocara
import context.home.domain.contract.CarSummaryInterface as CarSummaryRepository
CarSummaryRepository::all() // alias = nom simple du 1er wiring
```
```ocara
import context.home.domain.contract.CarSummaryInterface as SuperCarSummaryRepository
SuperCarSummaryRepository::all() // alias = nom simple du 2e wiring
```

### Règles de résolution, décidées (par l'utilisateur, avant implémentation)

- Une interface peut avoir **plusieurs** `wiring`.
- Import **sans alias** → construction/appel statique résolvent vers le
  **premier `wiring` déclaré, dans l'ordre textuel**.
- Import **avec alias** → l'alias doit correspondre exactement au **nom
  simple** d'un des `wiring` ; devient alors un alias transparent PARTOUT.
  Un alias sans correspondance est une erreur de compilation.
- Résolution **scopée à l'import concerné** (jamais mélangée entre
  interfaces différentes importées dans le même fichier).
- Le nom RÉEL de l'interface en annotation de type ordinaire reste le type
  abstrait — polymorphisme classique intact, substitution seulement dans
  les positions aujourd'hui dénuées de sens pour une interface nue.

### Autres points déjà tranchés (par l'utilisateur)

- Conformité obligatoire (`implements` + compatibilité de signature).
- Import implicite de chaque classe visée par un `wiring`.
- `wiring` valide uniquement dans une `interface`.
- Diagnostics obligatoires listés (repris dans `docs/diagnostics.md`, E38-E42).
- Portée v1 assumée : liaison 100% statique/compile-time, aucun conteneur
  DI au sens runtime, aucun changement de `wiring` selon l'environnement —
  limitation assumée, pas un oubli (voir `docs/EBNF.md` §17.1).

## Priorité / Complexité

**Terminé.** Confirmé « massif » comme anticipé : grammaire (mot-clé +
nouvelle règle répétable dans un corps `interface`, plus un prérequis non
anticipé — `public`/`static` en modificateurs de méthode d'interface),
lexer/parser, résolution d'imports (import implicite, problème d'ordre
résolu par un pré-scan dédié), sema (conformité étendue à la staticité,
5 nouveaux diagnostics), lower/codegen (un bug réel de dispatcher statique
découvert et corrigé en cours de route, orthogonal à `wiring` lui-même mais
atteignable uniquement depuis son prérequis de grammaire). Rien n'a été
coupé du périmètre décidé par l'utilisateur — le seul report est un bug
PRÉEXISTANT et SANS RAPPORT (dispatch d'instance sur variable
interface-typée, voir ci-dessus), explicitement documenté comme hors
périmètre plutôt que silencieusement laissé de côté.

## Fichiers clés

**Langage/compilateur** : `src/parsing/token.rs` (`TokenKind::Wiring`),
`src/parsing/lexer.d/tokenizer.d/keywords.rs` (mot-clé), `src/parsing/ast.d/interfaces.rs`
(`InterfaceMethod::is_static`, `WiringDecl`, `InterfaceDecl::wirings`),
`src/parsing/parser.d/declarations.rs` (`parse_interface`, `parse_wiring_decl`,
`parse_interface_method` avec `public?`/`static?`), `src/sema/symbols.d/types.rs`
(`InterfaceInfo::wirings`), `src/sema/symbols.d/registers.rs` (`register_interface`
avec `is_static` réel), `src/sema/error.rs` (`SemaError::InterfaceNoWiring`, E38),
`src/sema/typecheck.rs` (E38 dans `Expr::New`/`Expr::StaticCall`), `src/core/mod.rs`,
`src/core/interface_wiring.rs` (nouveau — `resolve_import_file_path`,
`collect_all_interfaces`, `resolve_bare_interface_names` + tests inline),
`src/core/alias_resolve.rs` (`compute_aliases` étendu, E41), `src/core/tests.rs`
(tests `compute_aliases` adaptés + nouveaux cas wiring), `src/lower/builder.d/interfaces.rs`
(`generate_interface_dispatchers` — saut des méthodes statiques, bug corrigé + tests),
`src/main.rs` (`enqueue_wiring_imports`, pré-scan `all_interfaces`, appel de
`resolve_bare_interface_names`, §4d/4d-bis étendues à `is_static`, nouvelle §4d-ter
E39/E40/E42).

**Tests** : `src/parsing/parser.d/tests.rs`, `src/core/tests.rs`,
`src/core/interface_wiring.rs` (module `tests` inline), `src/sema/tests/interface_wiring.rs`
(+ `src/sema/tests/mod.rs`), `src/lower/builder.d/interfaces.rs` (module `tests` inline),
`examples/60_interface_wiring.oc` (nouveau), `examples/tests/60_interface_wiringTest.oc`
(nouveau).

**Documentation** : `docs/EBNF.md` (§8 mots-clés, §17/§17.1, §31),
`docs/diagnostics.md` (E38-E42 + table des erreurs syntaxiques),
`tools/highlight/vsode/syntaxes/ocara.tmLanguage.json`.
