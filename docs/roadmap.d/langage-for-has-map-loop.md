# Boucle sur map : `for k has v in m` — implémenté (remplacement cassant)

Documentation utilisateur : `docs/EBNF.md` §27.3 et §31.

## Ce qui a été tranché

- **Remplacement cassant** : `for k => v in m` ne compile plus. Message :
  « 'for k => v in m' is no longer supported — write 'for k has v in m' ».
  `=>` reste réservé aux bras de `match`.
- **`has` mot-clé contextuel** : reconnu seulement entre la variable de boucle
  et `in` (`parse_for`, `src/parsing/parser.d/statements.rs`). Ailleurs, c'est
  un identifiant ordinaire : `m.has(k)`, `sess.has("user")`, ou même
  `var has:bool`.
- **Corpus migré** : toutes les boucles des exemples et des tests (8 fichiers
  `.oc`), `docs/EBNF.md`, `docs/builtins/Directory.md` et les commentaires du
  compilateur. Les fiches de roadmap déjà closes gardent l'ancienne forme,
  comme trace historique.
- **VS Code** : `has` est coloré comme mot-clé seulement dans `for k has v`,
  et son survol renvoie vers EBNF §27.3 (mot-clé contextuel, pas de survol sur
  `m.has(...)`).

## Fichiers clés

`src/parsing/parser.d/statements.rs`, `src/parsing/parser.d/tests.rs`,
`tools/highlight/vsode/syntaxes/ocara.tmLanguage.json`,
`tools/highlight/vsode/src/keywords.ts`.
