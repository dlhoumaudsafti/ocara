# La pédagogie (exemples numérotés + EBNF) a pris du retard sur le corpus `advanced/`

## ✅ Terminé

Les 3 points sont traités :

1. **`parent::` documenté** — nouvelle section §18.1 dans `docs/EBNF.md` (grammaire, exemple `Dog extends Animal` avec `parent::init(...)`/`parent::speak()`, note sur la différence de sémantique avec `self::`). `StaticCallee` et `PrimaryExpr` mis à jour aux 3 endroits où ils sont définis/dupliqués (§10, §23, §31 — vérifié par grep, les 3 occurrences de `StaticCallee ::=` sont identiques). `examples/12_inheritance.oc` **et** son test `examples/tests/12_inheritanceTest.oc` réécrits pour utiliser `parent::init(name, "Woof"/"Meow")` au lieu de redupliquer `self.name`/`self.sound` dans `Dog`/`Cat`. Vérifié : sortie du binaire compilé identique aux commentaires d'origine (`make regression 12_inheritance` → OK), les 6 assertions de `12_inheritanceTest.oc` passent, `ocaracs`/`--check` propres.
2. **Blocs runtime — pointeur ajouté plutôt qu'un nouvel exemple numéroté.** Décision consciente pour l'option "a minima" du ticket plutôt qu'un `34_runtime_blocks.oc` : la plage `34+` est déjà utilisée par `examples/tests/` pour des tests de régression bas niveau sans exemple numéroté correspondant (`34_string_nul_safetyTest.oc`, `35_var_auto_freeTest.oc`, ... jusqu'à `49_sqlite_with_open_raiseTest.oc`) — y ajouter un `examples/34_runtime_blocks.oc` pédagogique aurait créé une collision de numérotation trompeuse entre deux séries qui ne se correspondent plus après le 33. À la place, un encart a été ajouté dans `examples/README.md` juste avant la section "Site web complet", qui prévient explicitement que `advanced/httpserver/main.oc` (et les 3 autres apps `advanced/`) utilise les blocs `init`/`main`/`error`/`success`/`exit`, jamais vus dans la série 01-32, et renvoie vers `docs/EBNF.md` §5.
3. **Ligne `32_strict_operators.oc` de `examples/README.md` corrigée** — ne mentionne plus `===`/`!==`/`<==`/`>==` (supprimés en v0.2.0), décrit maintenant les comparaisons en toutes lettres.

## Constat

Une relecture complète de **tous** les fichiers de `examples/` (les 33 exemples numérotés, les ~26 démos de `builtins/`, `generics/`, `mods/`, `from/`, `project/`, et les 4 applications de `advanced/`) fait ressortir trois décalages concrets entre ce que le langage sait réellement faire et ce que la documentation/les exemples pédagogiques enseignent. Aucun des trois n'est un bug du compilateur — ce sont des trous de couverture documentaire, dans le même esprit que [coherence-documentation-ebnf-stdlib](coherence-documentation-ebnf-stdlib.md) (déjà clos), mais découverts en lisant le corpus d'exemples plutôt que l'EBNF elle-même.

### 1. `parent::` — un vrai appel au constructeur parent, invisible dans l'EBNF et dans l'exemple d'héritage

`parent::init()` (et plus généralement `parent::method()`) est une fonctionnalité réelle et implémentée : `src/lower/expr.d/lower.rs:864` et `:955` traitent spécifiquement l'appel `parent::method()` en y injectant `self` comme premier argument. Elle est utilisée de façon systématique dans **19 fichiers** des trois applications `advanced/` (`configs/Server.oc extends HTTPServer`, tous les composants `HTMLComponent`...), par exemple `examples/advanced/httpserver/configs/Server.oc:24`.

Pourtant :
- `docs/EBNF.md` §18 ("Héritage et implémentation") ne mentionne jamais `parent::` — grep à zéro résultat sur les 3634 lignes du document.
- L'exemple pédagogique dédié à l'héritage, `examples/12_inheritance.oc`, et l'exemple `examples/project/main.oc` (`class Student extends Score`) n'y font jamais appel : chaque sous-classe **redéclare manuellement** tous les champs du parent dans son propre `init` (`Dog`/`Cat` dupliquent `self.name`/`self.sound` au lieu d'appeler le constructeur d'`Animal`).

Un développeur qui apprend l'héritage via `12_inheritance.oc` n'a aucune raison de découvrir `parent::` avant de tomber dessus, sans explication, dans `advanced/`.

### 2. Deux structures de point d'entrée, une seule enseignée

Le langage a deux façons de structurer le point d'entrée d'un programme :
- `function main(): int { ... return 0 }` — utilisée sans exception dans les 33 exemples numérotés (`examples/01_variables.oc` → `examples/33_increment_decrement.oc`).
- Les blocs de cycle de vie `init` / `main` / `error` / `success` / `exit` avec les variables magiques `ERROR`/`SUCCESS` et le mot-clé `result` (§5 de l'EBNF, bien documentée formellement).

Les **quatre** applications de `examples/advanced/` (`mini_project/main.oc`, `httpserver/main.oc`, `tauri_httpserver/main.oc`, `game_sdl/game.oc`) utilisent exclusivement la seconde forme — aucune n'utilise `function main(): int`. Aucun des 33 exemples numérotés ne présente cette seconde forme. Un apprenant qui suit `examples/README.md` dans l'ordre saute donc directement d'une syntaxe jamais vue à du code "réel" qui n'utilise qu'elle.

### 3. `examples/README.md` décrit une syntaxe que l'exemple qu'il documente contredit

La ligne de `examples/README.md` pour `32_strict_operators.oc` dit : *« Opérateurs stricts : `===`, `!==`, `<==`, `>==`, `equal`, `not equal` »*. Or le fichier lui-même (`examples/32_strict_operators.oc:1-3`) affirme explicitement : *« Ocara v0.2.0 — toute comparaison s'écrit en toutes lettres, sans aucun symbole (plus de `==`, `!=`, `<=`, `>=`, `<`, `>`, `===`, `!==`, `<==`, `>==`) »*. La ligne d'index n'a pas été mise à jour lors du durcissement de syntaxe v0.2.0 qui a supprimé tous les opérateurs symboliques.

## Ce qui est demandé

1. Documenter `parent::` dans `docs/EBNF.md` §18, et faire au moins un exemple parmi `12_inheritance.oc`/`examples/project/` l'utiliser (ou ajouter un exemple dédié) plutôt que la duplication manuelle des champs.
2. Décider consciemment si les blocs `init`/`main`/`error`/`success`/`exit` doivent être introduits dans la série numérotée (un `34_runtime_blocks.oc` par exemple), ou a minima renvoyer explicitement depuis `examples/README.md`/`examples/advanced/*/README.md` vers §5 de l'EBNF avant de présenter le code `advanced/`.
3. Corriger la ligne `32_strict_operators.oc` de `examples/README.md`.

## Priorité / Complexité

**Terminé.** Était Priorité Basse, Complexité Légère — confirmé : corrections de documentation et d'exemples, aucun changement de compilateur. Note : `examples/project/main.oc` (`Student extends Score`) n'a **pas** été retouché — le ticket demandait de traiter "au moins un" exemple, `12_inheritance.oc` (l'exemple pédagogique dédié à l'héritage) étant le plus pertinent des deux.

## Fichiers clés

`docs/EBNF.md` (§10, §18.1 nouveau, §23, §31), `examples/12_inheritance.oc`, `examples/tests/12_inheritanceTest.oc`, `examples/README.md`.
