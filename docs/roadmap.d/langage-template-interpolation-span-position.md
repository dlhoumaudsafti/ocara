# Position erronée des diagnostics à l'intérieur d'une interpolation `${...}`

## Terminé — deux causes distinctes, corrigées séparément

Trouvé en creusant un signalement utilisateur : `ocara build` sur
`examples/advanced/mini_project_hexa` affichait `main.oc:1:1: error: 'message'
is 'consumed' and was already used at 16:30...` alors que le vrai problème
vivait dans `configs/components/Alert.oc:19:56` (un fichier importé
indirectement, jamais `main.oc`).

Écarté avant de conclure : ce n'est **pas** lié aux blocs `runtime core.X is
Y` — vérifié par reproduction croisée (un programme avec plusieurs blocs
`runtime` mais sans template literal affiche la bonne position ; un
programme avec template literal mais sans aucun bloc `runtime` — juste
`function main(): int` — affichait déjà la mauvaise ligne). Le nom du fichier
`src/core/runtime_expand.rs` (où vit une des deux causes) est un artefact
historique d'organisation du code, pas un indice de lien avec le mécanisme
`runtime` — la fonction en cause y est appelée pour absolument tout import,
avec ou sans bloc runtime.

## Cause 1 (la profonde) — le parser re-lexe/re-parse chaque `${...}` en isolation, sans jamais corriger les positions obtenues

`src/parsing/parser.d/expressions.rs`, cas `TokenKind::LitTemplate` : pour
chaque interpolation, le texte brut entre `${` et `}` est relu par un
`Lexer`/`Parser` flambant neufs, qui ne savent rien de leur position dans le
fichier d'origine — toute expression qui en résulte a donc des spans
commençant à (ligne 1, colonne 1) **du texte isolé**, jamais la vraie
position dans le fichier. Ce bug est structurel et touche TOUTE interpolation
de template dans tout le langage, indépendamment des imports — il était
simplement invisible jusqu'ici quand le diagnostic tombait dans le fichier
principal (la ligne/colonne étaient déjà fausses, mais le nom de fichier
"par coïncidence" correct masquait le problème).

Corrigé par un mécanisme en deux temps :
1. `src/parsing/token.rs` : `TemplatePart::ExprSrc` porte désormais aussi le
   `Span` du premier caractère de l'interpolation dans le fichier d'origine
   (capturé par le lexer, `src/parsing/lexer.d/scanner.d/readers.rs::read_template`,
   juste après avoir consommé `${`).
2. `src/parsing/ast.d/span_shift.rs` (nouveau) : `shift_expr_spans`/
   `shift_stmt_spans`, appelées juste après le re-parsing de l'interpolation
   (`expressions.rs`), traduisent chaque span relatif (au texte isolé) en
   position absolue à partir de cette origine — gère le cas multi-lignes
   (rare mais géré correctement : la colonne ne dépend de l'origine que sur
   la première ligne de l'interpolation).

Mirroir volontaire de `core::runtime_expand::update_expr_spans`/
`update_stmt_spans` (même liste de variants `Expr`/`Stmt`, même structure de
récursion) mais avec une opération de fond différente (décalage plutôt que
remplacement de `file`) — dupliqué plutôt que factorisé en un visiteur
générique commun aux deux : les deux fonctions ne tournent jamais au même
moment (celle-ci pendant le parsing, l'autre après, au chargement d'un
import) et généraliser une fonction existante déjà éprouvée sous pression de
temps aurait été plus risqué que de dupliquer une structure déjà stable.

## Cause 2 (celle qui rend le nom de FICHIER aussi faux) — récursion manquante dans `update_program_spans_with_file`

`src/core/runtime_expand.rs`, `update_expr_spans`, cas `Expr::Template { span,
.. }` : le `..` ignorait silencieusement le champ `parts` (les expressions
interpolées) — contrairement à tous les autres variants composites de cette
fonction (`Expr::Nameless`, `Expr::Match`, etc.), qui recursent bien dans
leurs enfants. Conséquence : le `span.file` d'une expression interpolée dans
un fichier chargé via `import` n'était jamais réattribué à ce fichier,
restant `None` — et `main.rs` retombe alors sur `args.input` (le fichier
passé en ligne de commande) pour l'affichage, d'où `main.oc`.

Corrigé en ajoutant la récursion manquante dans `parts` (variante
`TemplatePartExpr::Expr`), suivant exactement le même patron que les autres
variants de cette fonction.

## Vérifications

- Les deux repros manuels (fichier unique avec template literal ; import à
  deux niveaux `main.oc` → `Components.oc` → `Alert.oc` avec le même
  template) affichent désormais la bonne position dans le bon fichier —
  vérifié caractère par caractère sur `mini_project_hexa` :
  `Alert.oc:19:56` tombe exactement sur le `message` du second `${message}`.
- 6 nouveaux tests unitaires Rust : `src/parsing/ast.d/span_shift.rs`
  (décalage simple, décalage au milieu d'une expression, cas multi-lignes,
  arguments d'un appel imbriqué, template dans template) et
  `src/core/runtime_expand.rs` (le fichier est bien réattribué à une
  expression interpolée).
- `cargo test -p ocara` : 140 passed (134 avant ce ticket, +6), 0 failed.
- `cargo test -p ocara_runtime` : 105 passed, 7 ignored (MySQL, inchangé).
- `make build` avec `RUSTFLAGS="-D warnings"` : 0 warning, les 4 crates.
- `./ci/regression.sh` : tout passe.
- `./ci/unittests.sh examples/project/tests` : 50 PASS / 0 FAIL.
- `./ci/unittests.sh examples/tests` : 772 PASS / 0 FAIL / 0 ERREUR(S) (inchangé — ce correctif ne change le comportement d'aucun programme qui compilait déjà, seulement la précision des diagnostics).

## Priorité / Complexité

N'a jamais été mis en Priorité Haute/Moyenne de la roadmap : ce n'est pas un
comportement silencieusement faux ni un crash (les deux critères qui y
amènent les autres tickets de cette série), seulement un diagnostic
imprécis — traité directement dès sa découverte. **Complexité réelle**
Légère à Structurel : la cause 2 était un correctif d'une ligne, la cause 1
demandait de faire remonter une position à travers lexer → token → parser →
un nouveau module de décalage de spans — mais sans aucun changement de
syntaxe ni de grammaire.

## Fichiers clés

`src/parsing/token.rs` (`TemplatePart::ExprSrc` porte un `Span`),
`src/parsing/lexer.d/scanner.d/readers.rs` (`read_template` capture la
position d'origine), `src/parsing/parser.d/expressions.rs` (applique le
décalage après re-parsing), `src/parsing/ast.d/span_shift.rs` (nouveau),
`src/parsing/ast.d/mod.rs` + `src/parsing/ast.rs` (déclaration/export du
nouveau module), `src/core/runtime_expand.rs` (récursion manquante +
test), `src/core/render_file.rs` (adapté au nouveau type `ExprSrc` — les
templates de fichier `.html` n'ont pas de suivi ligne/colonne). Depuis le
2026-10-08, les expressions d'un gabarit `HTML::renderFile` sont décalées
sur l'appel (`shift_expr_spans`, qui reprend aussi le fichier de
l'origine) : un diagnostic de la sema pointe la ligne de l'appel dans son
fichier `.oc`, et non plus `fichier.oc:1:1`.
