# La pédagogie (exemples numérotés + EBNF) a pris du retard sur le corpus `advanced/`

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

**Priorité Basse** — n'affecte pas la correction du compilateur ni des binaires produits, mais affaiblit la crédibilité de la doc comme source de vérité et allonge inutilement la courbe d'apprentissage. **Complexité Légère** — corrections de documentation et d'exemples, aucun changement de compilateur.

## Fichiers clés

`docs/EBNF.md` (§18), `examples/12_inheritance.oc`, `examples/project/main.oc`, `examples/README.md`, `examples/advanced/*/main.oc` (référence de ce qui existe déjà et fonctionne).
