# Opérateurs d'affectation composés `+=`, `-=`, `*=`, `/=`, `%=` — implémenté

Documentation utilisateur : `docs/EBNF.md` §12.1, diagnostics E58 et E59.

## Ce qui a été tranché

- **Instruction seulement**, comme `x = e` (pas une expression).
- **Réécriture au parsing** en `x = x op e` (`Stmt::Assign`,
  `src/parsing/parser.d/compound_assign.rs`) : toutes les passes et le
  lowering réutilisent l'affectation existante (ownership, boxing `mixed`,
  champs, index).
- **Cibles** : variable, champ, élément indexé. Une cible qui contient un
  appel (`a[next()] += 1`) est refusée au parsing (**E58**), puisqu'elle
  serait évaluée deux fois.
- **Types** : ceux de `x op e`. `float += int` est donc refusé, comme
  `float + int`.
- **`-=` sur `string`** : `-=` produit `BinOp::Remove`. Sur une `string`, la
  sema note le site (`AstRewrites::string_removals`), et la passe post-sema
  (`core::named_args`) le réécrit en `String::replace(s, e, "")`, en ajoutant
  `import ocara.String` au besoin. Ailleurs, `Remove` devient `Sub`. Pas
  d'opérateur `-` binaire entre chaînes.

## Bug corrigé en passant (E59)

`-`, `*`, `/`, `%` (et `+` hors concaténation) acceptaient des `string`,
`bool`, `array` et `map` dès que les deux types étaient compatibles :
`"ab" - "b"` compilait et valait `-24` (différence des adresses). Ces
opérandes sont désormais refusés (**E59**). Aucun impact sur le corpus.

## Limite

`x -= e` dans le corps d'un `generic` instancié avec `T = string` reste une
soustraction : le corps est vérifié une seule fois avec `T` permissif.

## Fichiers clés

`src/parsing/token.rs`, `lexer.d/tokenizer.d/next_token.rs`,
`parser.d/compound_assign.rs`, `parser.d/statements.rs`,
`ast.d/expressions.rs` (`BinOp::Remove`), `src/sema/typecheck.rs`,
`src/sema/error.rs`, `src/sema/named_args.rs`, `src/core/named_args.rs`.
Tests : `src/sema/tests/compound_assign.rs`,
`examples/tests/76_compound_assignmentTest.oc`.
