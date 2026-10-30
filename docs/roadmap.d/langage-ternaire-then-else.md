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

Forme courte : un mot remplace `else <valeur>` et donne la valeur par défaut
du type de la branche `then` :

```ocara
var s:string = <condition> then <valeur> empty              // ""
var s:string|null = <condition> then <valeur> null          // null
var i:int = <condition> then <valeur> zero                  // 0
var f:float = <condition> then <valeur> zero                // 0.0
var a:array<int> = <condition> then <valeur> empty          // []
var m:map<string, int> = <condition> then <valeur> empty    // {}
```

`empty` et `zero` restent des identifiants ordinaires partout ailleurs :

```ocara
var zero:int = 1
var realzero:int = zero equal 0 then zero zero   // 0
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
CondExpr   ::= OrExpr ( "then" CondExpr ( "else" CondExpr | Default ) )?
Default    ::= "empty" | "zero" | "null"
```

- `a then b else c + d` se lit `a then b else (c + d)` : pour combiner le résultat, parenthéser (`(a then b else c) + d`), comme dans la demande.
- Chaînage sans parenthèses dans la branche `else` : `x then a else y then b else c` ≡ `x then a else (y then b else c)`. La branche `then` étant délimitée par `else`, elle accepte aussi un ternaire nu, mais les parenthèses restent recommandées (et seront proposées par `ocaracs`).
- `then` sans `else` ni valeur par défaut → erreur de parsing explicite (« `then` attend `else <valeur>`, `empty`, `zero` ou `null` »).
- **`empty` et `zero` sont des mots-clés contextuels**, pas des mots réservés : ils servent d'identifiants dans le code existant (`var empty:array<int>` dans `examples/08_arrays.oc`, `var zero:int` dans `examples/tests/15_operatorsTest.oc`, `String::empty(…)`, `m.zero()`). Le lexer les laisse en identifiants ; le parseur ne les lit comme valeur par défaut qu'**immédiatement après l'expression complète de la branche `then`**, là où un identifiant ne peut pas suivre une expression. Partout ailleurs (variable, branche `then` elle-même, nom de méthode après `.`/`::`), ils restent des identifiants : `zero equal 0 then zero zero` se lit condition `zero equal 0`, branche `then` la variable `zero`, valeur par défaut `zero`. Même principe que `has`, mot-clé seulement dans `for k has v in m`. `null` est déjà un mot-clé.
- Aucun programme valide ne change de sens : `c then v` sans suite était une erreur, donc lire `empty`/`zero`/`null` derrière, même à la ligne suivante, ne capte jamais une instruction qui compilait.
- `then v else empty` / `else zero` : **erreur**. Avec `else`, on écrit la valeur (`else ""`, `else 0`) ; la valeur par défaut n'existe que dans la forme courte, et `empty`/`zero` ne deviennent pas des expressions autonomes.
- Plusieurs lignes : le langage n'a pas de point-virgule, mais aucune instruction ne commence par `then` ni par `else` (le `else` d'un `if` suit toujours un `}`) ; le parseur peut donc accepter `then`/`else` en début de ligne, avec ou sans parenthèses. À couvrir par des tests de parseur, notamment `if … { x = c then a else b } else { … }`.
- Utilisable partout où une `Expression` est attendue : initialisation, affectation, argument, `return`, condition d'`if`/`while`, corps d'un bras de `match`, interpolation `${…}` d'un template string.

### Sémantique

- **Condition** : mêmes règles qu'`if` — `bool`, ou `mixed` évalué sur sa valeur (`false`, `0`, `null` sont faux, §11). Un autre type → nouveau diagnostic (prochain code libre, E72 à ce jour), documenté dans `docs/diagnostics.md`.
- **Évaluation paresseuse** : seule la branche choisie est évaluée (effets de bord, appels, `raise` compris).
- **Type** : l'union des types des deux branches, comme `match` (§26) — `c then "a" else null` est un `string|null`, `c then 1 else 2.0` un `int|float` (pas de promotion implicite, refusé là où un `int` est attendu).
- **Forme courte** : la valeur par défaut prend le type de la branche `then`, pas celui de la variable déclarée (la forme marche donc aussi en argument, en `return`, dans une interpolation) :
  - `empty` : `""` pour un `string`, `[]` pour un `array<T>`, `{}` pour un `map<K, V>` ; type du résultat = type de la branche ;
  - `zero` : `0` pour un `int`, `0.0` pour un `float` ; type du résultat = type de la branche ;
  - `null` : tout type ; type du résultat = `T|null` ;
  - erreur de compilation (nouveau code) : `empty`/`zero` sur un type qui n'en a pas (`zero` sur un `string`, `empty` sur un `int`, une classe utilisateur), ou sur une branche `mixed` ou union (`int|string`, `string|null`) : on ne sait pas quelle valeur produire. Le message propose `else <valeur>` ou `null`.
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

- **Extension VS Code** (`tools/highlight/vsode/syntaxes/ocara.tmLanguage.json`) : `then` dans les mots-clés de contrôle, à côté de `else` (`keyword.control.ocara`). **`empty`/`zero`/`null` de la forme courte de la même couleur que `then`/`else`**, et seulement à cette place : `var zero`, `String::empty(…)`, `m.zero()` gardent la couleur d'un identifiant. Une regex TextMate ne voit pas la fin d'une expression : la position exacte vient des jetons sémantiques du serveur de langage (`src/lsp/semantic.rs`, jeton `keyword` sur la valeur par défaut, connue par l'AST), la grammaire TextMate se limitant à un repli prudent (ne rien colorer plutôt que colorer une variable `zero`). `null` est déjà coloré. Lire le README, désinstaller, recompiler sans changer la version, réinstaller.
- **ocaracs** : mise en forme multi-ligne (`then`/`else` indentés sous la condition, comme dans la demande) ; suggestion de parenthèses autour d'un ternaire nu dans une branche `then` ou combiné par un opérateur.
- **Tree-sitter** (ticket `tooling-tree-sitter.md`) : à inclure dans la grammaire quand elle sera écrite.

## Tests et exemple

- Tests de parseur (`src/parsing/parser.d/tests.rs`) : précédence (`a then b else c + d`), associativité à droite, parenthèses, multi-ligne avec et sans parenthèses, `then` sans `else`, forme courte (`empty`/`zero`/`null`, y compris à la ligne suivante), `zero equal 0 then zero zero`, `empty`/`zero` toujours utilisables comme variables et noms de méthode, `then v else empty` refusé, ternaire dans une condition d'`if` et dans un bras de `match`.
- Tests sema : condition non `bool` (nouveau code), type union des branches, valeur par défaut selon le type de la branche (`zero` → `0.0` sur un `float`, `empty` sur `array`/`map`, `null` → `T|null`) et refus sur un type sans valeur par défaut, `mixed` ou union, raffinement `is`/`not equal null`.
- Exemple de régression `examples/` + test ocaraunit : chaque forme de la demande, évaluation paresseuse (une branche qui ferait `raise` n'est pas exécutée), interpolation `${c then "oui" else "non"}`, ternaires imbriqués produisant des chaînes et des objets, sans fuite.

## Documentation

- `docs/EBNF.md` : §10 (règle `CondExpr`), §11 (nouvelle ligne de précédence, niveau 0), nouvelle sous-section dédiée avec les exemples ci-dessus, §31 (grammaire complète) à l'identique.
- `docs/diagnostics.md` : nouveau code pour la condition non booléenne, nouveau code pour une valeur par défaut impossible (`empty`/`zero` sur un type qui n'en a pas), erreur de parsing `then` sans `else` ni valeur par défaut.
- `docs/EBNF.md` : la forme courte et la règle des mots-clés contextuels, dans la sous-section dédiée.

## Complexité

**Légère à Structurel** — la syntaxe et le typage sont simples, mais le nouveau nœud traverse une vingtaine de passes et l'abaissement doit respecter le contrat de propriété des valeurs (zone mémoire : tests de fuite obligatoires).
