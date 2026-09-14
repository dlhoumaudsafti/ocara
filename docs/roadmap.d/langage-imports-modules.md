# Résolution des imports : deux chemins redondants et incohérents

## ✅ Corrigé

## Constat (avant correctif)

La résolution des imports vit intégralement dans `src/main.rs` (~350 lignes), pas dans un module dédié — en décalage avec `docs/workflow-compilation.md`. Pour le format d'import ancien (`import module.Path`), il existait **deux chemins de chargement redondants** :

1. Un chemin récursif correct (src/main.rs, section 4a) qui fusionne classes/interfaces/modules/generics et respecte la résolution par namespace.
2. Un second chemin entièrement redondant (section 4b) qui relisait les mêmes fichiers mais ne fusionnait que classes/functions/consts (**interfaces et modules en étaient absents**), sans récursion vers les imports du fichier chargé, et sans résolution par namespace (construction de chemin naïve).

Masqué tant que le premier chemin traitait déjà le cas avant que le second n'agisse — mais un bug latent pour des cas plus complexes (fichiers namespacés, classes chargées uniquement via le second chemin).

**Confirmé concrètement** : `examples/project/tests/mainTest.oc` (`import main`, ancien format à un seul segment) échouait à la compilation avec `interface 'Printable' not found`. Ce cas précis résolvait au symbole **fonction** `main` du fichier via le premier chemin (recherche par nom, `main` étant bien enregistré comme une fonction), puis c'est le **second chemin redondant** qui fusionnait réellement les classes `Score`/`Student` du fichier (fusion inconditionnelle de tout `mod_prog.classes`, sans regarder ce qui a été demandé) — sans jamais fusionner `Printable`/`Comparable`, qui n'existaient que dans `mod_prog.interfaces`.

## Correctif

Le second chemin (`src/main.rs`, ancienne section "4b. Chargement et fusion des modules utilisateur") est supprimé — chaque `import module.Path` (ancien format) est déjà converti en import virtuel "from" et traité par le chemin récursif unique, qui gère correctement classes/generics/interfaces/modules/functions, l'alias et la résolution par namespace.

**Lacune annexe découverte en supprimant le second chemin** : le chemin unique restant ne rapatriait jamais les **constantes de premier niveau** (`const X:T = ...`) du fichier importé lors d'un import sélectif (`import Circle from "file"`) — seul `import * from "file"` (wildcard) le faisait déjà. Une classe/méthode important référençant une const de son fichier d'origine (ex. `Score::is_passing()` lisant `PASS_MARK`, une const top-level de `main.oc`) cassait avec `undefined symbol 'PASS_MARK'`. **Corrigé** : un import sélectif rapatrie désormais aussi toutes les consts du fichier source non déjà présentes (même logique que pour les interfaces `implements`, déjà rapatriées automatiquement).

`examples/project/tests/mainTest.oc` s'appuyait sur le bug du second chemin pour rapatrier `Score`/`Student`/`Color` via son seul `import main` — corrigé pour utiliser la syntaxe documentée (EBNF §4.3, "un fichier = un symbole") : `import Score from "main"`, `import Student from "main"`, `import Color from "main"` ajoutés à côté de `import main` (la fonction).

Vérifié : `examples/project/tests/mainTest.oc` compile et passe ses 27 assertions (0 auparavant, faute de compiler). `make regression` : **386 PASS / 0 FAIL** côté ocaraunit, et surtout **49 PASS / 0 FAIL / 0 ERREUR(S)** côté tests projet (contre 22 PASS / 1 ERREUR avant) — c'est la première fois que `make regression` se termine sans une seule erreur de compilation dans toute la suite.

## Fichiers clés

`src/main.rs` (section 4a, boucle `imports_to_process` — résolution par symbole + rapatriement des consts), `examples/project/tests/mainTest.oc`.
