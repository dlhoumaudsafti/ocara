# Cohérence interne de l'EBNF et de la stdlib — la doc grandit par accrétion sans passe de relecture

## Statut

La première passe est terminée (voir §"Ce qui a été fait" plus bas). **Rouvert le 2026-10-13** : l'audit de la roadmap a relevé de nouvelles divergences, listées dans « Reste à faire » en fin de fiche.

## Constat général

Le noyau du langage est cohérent en pratique (tous les exemples lus utilisent la même syntaxe sans divergence). C'est la **documentation de référence** (`docs/EBNF.md`, `docs/builtins/*.md`) qui accumule des incohérences internes au fil des sections ajoutées une à une, sans passe de convergence finale — aucune ne casse le compilateur, mais chacune érode la confiance qu'on peut accorder à la spec comme source de vérité, ce qui compte pour la stabilité perçue du langage autant que l'absence de bugs runtime.

## EBNF — incohérences internes vérifiées

1. **`Program` a deux définitions différentes dans le même fichier.** `docs/EBNF.md:79-83` (§2, "Structure d'un programme") inclut `GenericDecl` dans l'alternation top-level. `docs/EBNF.md:3429-3434` (§31, "Grammaire EBNF complète" — censée être LA référence canonique) **l'omet**. `GenericDecl` est bien définie deux fois (`docs/EBNF.md:2422` et `:3462`) mais devient, à la lettre de la grammaire "complète", inaccessible depuis `Program`.
2. **`ModuleDecl`** est référencée dans `Program` (les deux sections) mais sa règle (`docs/EBNF.md:2329`, `"module" Identifier ClassBody`) n'est définie qu'en §19 — jamais rapatriée dans la section "grammaire complète", contrairement à la plupart des autres règles qui y sont dupliquées.
3. **Noms de règles instables entre sections** : §27.2/27.3 nomment deux règles distinctes `ForInStmt` (`docs/EBNF.md:3152`) et `ForMapStmt` (`:3164`) ; §31 les fusionne en une seule règle `ForStmt` à deux alternatives (`:3558-3560`) sans jamais utiliser ces deux noms.
4. **`Function<T(...)>` documenté comme obligatoire, contredit par 10 exemples.** §6.5 (`docs/EBNF.md:897-899`) : *« La syntaxe avec parenthèses est **obligatoire** depuis la version 0.1.0 »*, avec une grammaire stricte (`docs/EBNF.md:894`) qui n'admet que `Function<Type(...)>`. Pourtant `Function` **nu, sans aucun générique**, est utilisé comme type dans au moins 10 exemples des sections voisines (`docs/EBNF.md:1746,1755,1770,1775,1786,1799,1822,1828,1835,3014-3015` — §14.3 "Fonctions de première classe", §16.6, §23). La note historique (`:907`) ne documente que la suppression de l'ancienne forme `Function<ReturnType>` sans parenthèses, jamais celle de la forme totalement nue — les exemples n'ont simplement jamais été mis à jour après le durcissement de syntaxe de §6.5.

## Stdlib — incohérences sémantiques entre modules jumeaux

1. **`Map::get` vs le reste de `Map.md`** : la section propre de `Map::get` (`docs/builtins/Map.md:55-60`) dit *« Si la clé est absente, le comportement dépend du runtime (retourne `0` / chaîne vide par défaut) »* — formulation vague qui ne s'engage même pas sur un comportement précis. Plus loin (`docs/builtins/Map.md:363-368`), la doc énumère les méthodes qui *« ne lèvent jamais d'exception »* (`has`, `set`, `remove`, `size`/`isEmpty`, `keys`/`values`, `merge`) — `get` est absente de cette liste, ce qui laisse penser par omission qu'elle PEUT lever une exception, sans jamais le confirmer explicitement dans sa propre section. Les deux passages du même document ne racontent pas la même histoire.
2. **`String::replace` vs `Regex::replace` — même nom, sémantique opposée.** `String::replace(s, from, to)` (`docs/builtins/String.md:82-84`) remplace **toutes** les occurrences (exemple vérifié : `"chat noir chat blanc"` → `"chien noir chien blanc"`). `Regex::replace(pattern, s, repl)` (`docs/builtins/Regex.md:60-62`) remplace **uniquement la première** occurrence (exemple vérifié : `"ref-123-abc-456"` → `"ref-NUM-abc-456"`, le second nombre non touché) — `Regex::replaceAll` existe séparément pour l'équivalent de `String::replace`. Un développeur qui connaît `String::replace` et passe à `Regex::replace` en s'attendant au même comportement introduit un bug silencieux.
3. **`queryOne` — sentinelle d'absence différente entre `SQLite` et `MySQL`.** `db.queryOne(query) → map<string, mixed>` (`docs/builtins/SQLite.md:54`) retourne une **map vide** si aucun résultat (`Map::size(user) > 0` dans l'exemple). `db.queryOne(query) → map<string, mixed>|null` (`docs/builtins/MySQL.md:71`) retourne **`null`** si aucun résultat (`user not equal null` dans l'exemple). Ce sont deux builtins jumeaux (même rôle, même nom de méthode) avec deux conventions différentes pour représenter "pas de résultat" — piège garanti pour quiconque écrit du code générique sur les deux. Accessoirement, l'exemple MySQL lui-même (`docs/builtins/MySQL.md:76`) déclare `const user:map<string, mixed> = db.queryOne(...)` — un type non-nullable pour une méthode dont la signature documentée juste au-dessus est `|null` : l'exemple ne correspond pas à sa propre signature.

## Ce qui a été fait

Corrections ciblées, aucun changement de comportement du compilateur :

**EBNF (4 points) :**
1. §31 ("grammaire complète") ré-inclut `GenericDecl` dans la règle `Program`, désormais identique à §2.
2. `ModuleDecl ::= "module" Identifier ClassBody` rapatriée dans §31 (juste après `GenericDecl`), là où elle manquait.
3. §27.2/27.3 renommées `ForInStmt`/`ForMapStmt` → `ForStmt` (les deux alternatives d'une seule règle), identique au nom utilisé en §31 et dans `Statement`.
4. Les 10 exemples utilisant `Function` nu (§14.3, §23) corrigés vers la syntaxe `Function<ReturnType(ParamTypes)>` obligatoire depuis 0.1.0 (vérifié contre `src/parsing/parser.d/types_parsing.rs:68-89` : le parser exige `<` immédiatement après `Function`, aucun chemin de repli).

**Stdlib (3 points) :**
1. `docs/builtins/Map.md` : la section `Map::get` disait à tort *« le comportement dépend du runtime »* — vérifié contre `runtime/src/lib.rs:1496-1521` (`Map_get`) : lève toujours `MapException` (code 101, `KEY_NOT_FOUND`) si la clé est absente, cohérent avec la table des codes d'erreur du même document. Section réécrite, note explicite ajoutée à la liste "ne lève jamais d'exception" pour que `get` n'y manque plus par omission.
2. `docs/builtins/String.md`/`docs/builtins/Regex.md` : avertissement croisé ajouté dans les deux sections `replace` (sémantiques opposées — "toutes occurrences" vs "première occurrence seulement" — sous le même nom de méthode). Pas d'unification des deux comportements (changement d'API, hors périmètre "Simple").
3. `docs/builtins/SQLite.md`/`docs/builtins/MySQL.md` : avertissement croisé ajouté dans les deux sections `queryOne` (map vide vs `null` pour "aucun résultat"). Corrigé au passage le type de l'exemple MySQL (`const user:map<string, mixed>` → `map<string, mixed>|null`, qui ne correspondait pas à sa propre signature documentée juste au-dessus).

**Processus :** nouvelle étape ajoutée à `docs/roadmap.md` § "Méthode de travail" — après toute modification de `docs/EBNF.md`, relire §31 pour vérifier qu'il reste la source unique et à jour.

Vérifié : corrections de texte uniquement, aucun fichier source/runtime touché — `make build`/`make regression` non ré-exécutés pour ce ticket (aucune modification de comportement possible).

## Priorité / Complexité

**Terminé.** **Complexité réelle : Simple**, comme estimé — corrections de texte ciblées, pas de changement de comportement du compilateur.

## Fichiers clés

`docs/EBNF.md` (§2, §6.5, §14.3, §16.6, §19, §23, §27, §31), `docs/builtins/Map.md`, `docs/builtins/String.md`, `docs/builtins/Regex.md`, `docs/builtins/SQLite.md`, `docs/builtins/MySQL.md`, `docs/roadmap.md` (méthode de travail).

## Reste à faire — audit du 2026-10-13

### EBNF

- §31 (« Grammaire EBNF complète ») : `Program` impose un ordre strict et ne contient pas `ArchitectureDecl` (§33) — le parseur accepte n'importe quel ordre après `namespace`, et `when … architecture` ; `Statement` n'inclut pas `ResultStmt` (§5.5) ; `TemplateString` y est utilisé sans être défini (il l'est en §7, ~l.1014) ; `Identifier ::= [a-zA-Z]…` alors que le lexer accepte l'Unicode et un `_` initial.
- §11 (~l.1396) présente encore `..` comme opérateur de plage (refusé par le parseur, remplacé par `to`/`until`).
- §8 (mots réservés) oublie `emit`, `when`, `message`, `smaller`, `greater`.
- Table variadic (~l.1730) : `variadic<Function<int>>` est une erreur de syntaxe (`Function<int()>`).
- ~l.1814 renvoie vers « §4.5 », qui n'existe pas (§6.5).
- ~l.3770 : les exceptions builtin « héritent implicitement de `Exception` » — faux pour la sema (bug suivi dans [sema-builtin-parent-init-noop](sema-builtin-parent-init-noop.md)).

### `docs/builtins/*.md`

- Exemples qui ne compilent pas : 15 `var m:map = …` sans paramètres de type dans `Map.md` ; `Map::size(user) > 0` dans `SQLite.md` (~l.93) et `MySQL.md` (~l.99) ; boucles `while i < …` dans `Convert.md`, `Mutex.md`, `Regex.md` (opérateurs `<`/`>` retirés).
- Méthodes non documentées : `UnitTest::assertRaises`, `assertException{Code,Message,Source}{Not}Equals`, `assertArray`/`assertMap`/`assertClass`/`assertEnum`/`assertFunction` ; `Array::get`/`Array::set` ; `HTMLComponent.tag` ; SDL `unloadTexture`/`unloadSound`/`unloadFont` ; aucune page pour les classes d'exception (`src/builtins/exception.rs`).
- `HTTPServer.md` (~l.545) : affirmation fausse sur les captures partagées (voir [runtime-httpserver-race-condition](runtime-httpserver-race-condition.md)).

### Autres

- 13 liens vers des fiches supprimées (`langage-mixed-arithmetic`, `memoire-strategie-var`, `langage-generiques`, `memoire-concurrence-threads`…) dans `docs/builtins/Map.md`, `YAML.md`, `examples/tests/{33,35,36,39,40,44}`, `src/lower/**`, `src/sema/**`, `runtime/src/*.rs`.
- Commentaires d'exemples devenus faux (bugs corrigés depuis) : `examples/61_interface_instance_dispatch.oc` (~l.73-77) et `examples/tests/61_…` (~l.98-107), `examples/tests/52_…` (~l.44-48), `examples/tests/47_…` (en-tête : « le cas boucle reste cassé »).
- `examples/01_variables.oc` (~l.10) et `docs/variables.md` (~l.173) décrivent encore la constante globale comme « évaluée à la compilation ».
- `docs/benchmarking.md` (~l.192) : ancienne syntaxe `0..N`.
- `ocara --help` : `--dump` affiche aussi le HIR, pas seulement tokens et AST.
- `README.md` (~l.3) : badge vers `github.com/yourusername`.
- Extension VS Code : `layers` et `shared` (clés du bloc `architecture`) ne sont pas colorés.
- `examples/advanced/mini_project` : commentaires affirmant qu'aucun parseur `x-www-form-urlencoded` n'existe (`configs/routes/Cars.oc`, `controllers/CarController.oc`) — il existe depuis [stdlib-httpserver-post-body-parsing](stdlib-httpserver-post-body-parsing.md).
- Commentaires de code périmés citant des mécanismes supprimés par le comptage de références (`FieldOwnership::Resource` dans `src/sema/typecheck.rs`, `scope.rs`, `error.rs` ; `__value_free` dans `src/codegen/desc.d/lowlevel.rs`).

Validation suggérée : compiler automatiquement les blocs ```` ```ocara ```` de `docs/` (au moins ceux marqués comme complets) dans `make regression`.

**Basse — Simple** (aucun impact sur les binaires produits, mais la doc induit en erreur).
