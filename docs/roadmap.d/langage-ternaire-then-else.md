# Expression conditionnelle `<condition> then <valeur> else <valeur>`

## Demande (David)

Une nouvelle syntaxe d'expression ternaire, en toutes lettres comme les comparaisons (§11.1) :

```ocara
var s:string = <condition> then <valeur> else <valeur>
```

Imbrication libre, et parenthèses pour isoler le résultat avant de le combiner :

```ocara
var s:string = (<condition> then <valeur> else <valeur>) + <valeur concaténée>
```

Écriture sur plusieurs lignes :

```ocara
var s:string = (<condition>
  then <valeur>
  else <valeur>
) + <valeur concaténée>
```

Ternaire imbriqué dans une branche :

```ocara
var s:int = (<condition>
  then (<condition>
    then <valeur>
    else <valeur>
  )
  else <valeur>
) + <valeur ajoutée>
```

## État actuel (vérifié dans le code)

- `then` n'est **pas** un mot-clé : absent de `src/parsing/lexer.d/tokenizer.d/keywords.rs` (où `else` existe déjà, `TokenKind::Else`). Aucun exemple `.oc` ni aucune doc de builtin ne l'emploie comme identifiant ou nom de méthode : le réserver ne casse rien de connu.
- La seule expression conditionnelle existante est `match` (§26), dont le type est l'union des types de ses bras.
- Le parseur d'expressions (`src/parsing/parser.d/expressions.rs`) commence à `parse_or` : `parse_expr` → `parse_or` → `parse_and` → … Il existe une variante `parse_or_no_is` pour les bras de `match`.
- `and`/`or` sont déjà abaissés en blocs avec court-circuit (`src/lower/expr.d/lower.rs`, autour de `BinOp::And`) : même mécanique à reprendre.

## Spécification proposée

### Grammaire

Niveau de précédence le plus faible, sous `or`, associatif à droite :

```ebnf
Expression ::= CondExpr
CondExpr   ::= OrExpr ( "then" CondExpr "else" CondExpr )?
```

- `a then b else c + d` se lit `a then b else (c + d)` : pour combiner le résultat, parenthéser (`(a then b else c) + d`), comme dans la demande.
- Chaînage sans parenthèses dans la branche `else` : `x then a else y then b else c` ≡ `x then a else (y then b else c)`. La branche `then` étant délimitée par `else`, elle accepte aussi un ternaire nu, mais les parenthèses restent recommandées (et seront proposées par `ocaracs`).
- `then` sans `else` → erreur de parsing explicite (« `then` attend une branche `else` »).
- Plusieurs lignes : le langage n'a pas de point-virgule, mais aucune instruction ne commence par `then` ni par `else` (le `else` d'un `if` suit toujours un `}`) ; le parseur peut donc accepter `then`/`else` en début de ligne, avec ou sans parenthèses. À couvrir par des tests de parseur, notamment `if … { x = c then a else b } else { … }`.
- Utilisable partout où une `Expression` est attendue : initialisation, affectation, argument, `return`, condition d'`if`/`while`, corps d'un bras de `match`, interpolation `${…}` d'un template string.

### Sémantique

- **Condition** : mêmes règles qu'`if` — `bool`, ou `mixed` évalué sur sa valeur (`false`, `0`, `null` sont faux, §11). Un autre type → nouveau diagnostic (prochain code libre, E72 à ce jour), documenté dans `docs/diagnostics.md`.
- **Évaluation paresseuse** : seule la branche choisie est évaluée (effets de bord, appels, `raise` compris).
- **Type** : l'union des types des deux branches, comme `match` (§26) — `c then "a" else null` est un `string|null`, `c then 1 else 2.0` un `int|float` (pas de promotion implicite, refusé là où un `int` est attendu).
- **Raffinement** : comme `if`, la condition raffine les types dans les branches — `x is string then x.length() else 0`, `u not equal null then u.name else ""` (§6.3, `src/sema/narrowing.rs`).
- **Mémoire** : la valeur produite est possédée par le résultat quelle que soit la branche (même contrat qu'un bras de `match`) : aucune fuite, aucune double libération de la branche non prise. Vérifier avec `--leak-check` sur des branches qui produisent des chaînes, tableaux et objets temporaires, et des ternaires imbriqués.

## Implémentation (pistes)

1. **Lexer** : `"then" => TokenKind::Then` dans `keywords.rs`.
2. **AST** : `Expr::Cond { cond, then_expr, else_expr, span }` dans `src/parsing/ast.d/expressions.rs` (+ `span()`, `span_shift.rs`).
3. **Parseur** : `parse_cond` au-dessus de `parse_or` (et sa variante `_no_is` pour les bras de `match`), avec `deepen()` pour la limite `MAX_EXPR_DEPTH`.
4. **Toutes les passes qui parcourent `Expr`** : chaque fichier qui traite aujourd'hui `Expr::Match` doit traiter `Expr::Cond` (≈ 20 fichiers : `grep -rln "Expr::Match" src`) — notamment `sema/typecheck.rs`, `sema/escape.rs`, `sema/narrowing.rs`, `core/monomorph.rs`, `core/alias_resolve.rs`, `core/named_args.rs`, `core/interface_wiring.rs`, `core/render_file.rs`, `lower/expr.d/{lower,typeinfer,captures,nameless_scan}.rs`, `lower/stmt.d/ownership.rs`, `lower/builder.d/borrow_scan.rs`, `lower/stmt.d/statements.d/map_loop_scan.rs`. Les captures de `nameless` dans une branche doivent être vues.
5. **Abaissement** : blocs `then`/`else` + bloc de fusion à paramètre (comme `and`/`or`), avec le boxing en `mixed` quand les branches ont des types différents (même chemin que `match`).
6. **Serveur de langage** : survol/définition dans les branches, mot-clé dans la complétion.

## Outillage

- **Extension VS Code** (`tools/highlight/vsode/syntaxes/ocara.tmLanguage.json`) : `then` dans les mots-clés de contrôle, à côté de `else`. Lire le README, désinstaller, recompiler sans changer la version, réinstaller.
- **ocaracs** : mise en forme multi-ligne (`then`/`else` indentés sous la condition, comme dans la demande) ; suggestion de parenthèses autour d'un ternaire nu dans une branche `then` ou combiné par un opérateur.
- **Tree-sitter** (ticket `tooling-tree-sitter.md`) : à inclure dans la grammaire quand elle sera écrite.

## Tests et exemple

- Tests de parseur (`src/parsing/parser.d/tests.rs`) : précédence (`a then b else c + d`), associativité à droite, parenthèses, multi-ligne avec et sans parenthèses, `then` sans `else`, ternaire dans une condition d'`if` et dans un bras de `match`.
- Tests sema : condition non `bool` (nouveau code), type union des branches, raffinement `is`/`not equal null`.
- Exemple de régression `examples/` + test ocaraunit : chaque forme de la demande, évaluation paresseuse (une branche qui ferait `raise` n'est pas exécutée), interpolation `${c then "oui" else "non"}`, ternaires imbriqués produisant des chaînes et des objets, sans fuite.

## Documentation

- `docs/EBNF.md` : §10 (règle `CondExpr`), §11 (nouvelle ligne de précédence, niveau 0), nouvelle sous-section dédiée avec les exemples ci-dessus, §31 (grammaire complète) à l'identique.
- `docs/diagnostics.md` : nouveau code pour la condition non booléenne, erreur de parsing `then` sans `else`.

## Complexité

**Légère à Structurel** — la syntaxe et le typage sont simples, mais le nouveau nœud traverse une vingtaine de passes et l'abaissement doit respecter le contrat de propriété des valeurs (zone mémoire : tests de fuite obligatoires).
