# Opérateurs d'incrémentation/décrémentation (`i++`, `++i`, `i--`, `--i`)

## Décision de conception

**Option B retenue** (sémantique complète façon C : `i++`/`++i` sont des EXPRESSIONS à part entière, utilisables partout où une expression est attendue — `x = i++`, `foo(i++)`, `arr[i++]`... — avec une valeur de retour qui diffère entre pré- et post-forme). Ceci est un **changement structurel** : nouveau nœud AST, nouvelles règles de grammaire aux niveaux `UnaryExpr`/`PostfixExpr`, nouvelle validation sémantique, et une lowering avec effet de bord (store) au milieu d'une expression — zone plus délicate que le sucre pur envisagé en Option A.

## État actuel du langage (confirmé par lecture du code)

- **Lexer** (`src/parsing/lexer.d/tokenizer.d/next_token.rs`) : `+`/`-` sont reconnus caractère par caractère, sans lookahead — contrairement à `=`/`<`/`>` qui, eux, captent déjà `==`/`<=`/`>=`. Aucun token `PlusPlus`/`MinusMinus` n'existe dans `TokenKind` (`src/parsing/token.rs`).
- **Grammaire** (`docs/EBNF.md`) : `UnaryExpr ::= ( "not" | "-" | "resolve" ) UnaryExpr | PostfixExpr`, `PostfixExpr ::= PrimaryExpr ( CallSuffix | IndexSuffix | FieldSuffix )*`. Aucune place actuelle pour `++`/`--` (préfixe ou suffixe). `+` unaire n'existe pas du tout (donc `++i` n'est actuellement pas un programme valide, pour aucune raison — zéro conflit avec l'existant côté préfixe).
- **`Stmt::Assign { target: Expr, value: Expr, span }`** (`src/parsing/ast.d/statements.rs`) : le PARSER ne restreint PAS la forme de `target` — il parse une `Expr` générique (`parse_stmt`, `src/parsing/parser.d/statements.rs` lignes ~52-61 : `let expr = self.parse_expr()?; if ... Eq ... Stmt::Assign`). C'est la SÉMA qui valide après coup que `target` est bien un `Ident`/`Field`/`Index` (`src/sema/typecheck.rs` lignes 786-822, cas `_ => SemaError::InvalidAssign`). **Ce même schéma (parser permissif, sema qui valide la forme lvalue) est le patron à reproduire pour `IncDec`.**
- **`lower_assign`** (`src/lower/stmt.d/statements.d/assignments.rs`) : logique de stockage déjà écrite pour les 3 formes de cible :
  - `Ident` : boxing éventuel vers `mixed` (`box_for_any`) + libération de l'ancienne valeur si `scoped`/`consumed` (`free_before_reassign`) + `store_local`.
  - `Field` : résolution de la classe (`var_class`/`current_class`/chaînage via `resolve_chained_field_class`) → offset → `Inst::SetField`.
  - `Index` : distinction array/map (`map_vars`/`class_map_fields`) → `__array_set`/`__map_set`.
  Cette fonction lowere `value` puis stocke — mais NE RETOURNE RIEN (c'est un statement). Pour `IncDec`, il faut la même logique de stockage, mais en réutilisant une valeur DÉJÀ CALCULÉE (`ancien ± 1`) plutôt qu'un `Expr` à lowered, et en retournant une `Value` (l'ancienne ou la nouvelle selon pré/post) — voir la section Lowering plus bas.
- **Pas de boucle `for(init; cond; incr)` C-style** — seul usage réel : corps de `while`/`for..in`, ou expression libre. N'empêche pas l'implémentation, juste un rappel que le cas d'usage "en-tête de boucle" n'existe pas ici.
- **Risque de compatibilité vérifié** : introduire `--` comme token unique change la lecture de `--x` (sans espace) — aujourd'hui `Neg(Neg(x))` via la règle récursive `UnaryExpr ::= "-" UnaryExpr`. Recherche exhaustive dans `examples/` : **aucun programme actuel n'utilise cette forme adjacente**. `- -x` (avec espace) continue de fonctionner sans changement (deux tokens `Minus` séparés). Risque réel mais nul en pratique, comportement identique à l'arbitrage historique de C.

## Conception détaillée

### 1. AST — nouveau nœud

```rust
// src/parsing/ast.d/expressions.rs
pub enum IncDecOp { PreInc, PreDec, PostInc, PostDec }

// Nouveau variant de Expr :
IncDec { op: IncDecOp, target: Box<Expr>, span: Span }
```

Un seul nœud paramétré par un enum à 4 valeurs plutôt que 4 variants `Expr` séparés ou des booléens (`is_prefix`, `is_inc`) — plus lisible aux points de match (`match op { PreInc | PostInc => Add, ... }`, `match op { PreInc | PreDec => new_val, PostInc | PostDec => old_val }`).

`target` : `Box<Expr>`, générique à ce niveau (comme `Stmt::Assign::target`) — la restriction de forme (`Ident`/`Field`/`Index` uniquement) est portée par la SÉMA, pas par le parser, pour rester cohérent avec le patron existant. Une chaîne absurde comme `++(i++)` ou `(-x)++` PARSE syntaxiquement (récursion générique de la grammaire, voir point 2) mais est REJETÉE en sema (le `target` imbriqué n'est ni `Ident`, ni `Field`, ni `Index`).

### 2. Grammaire (EBNF + parser)

```
UnaryExpr   ::= ( "not" | "-" | "resolve" | "++" | "--" ) UnaryExpr | PostfixExpr
PostfixExpr ::= PrimaryExpr ( CallSuffix | IndexSuffix | FieldSuffix | "++" | "--" )*
```

- **Préfixe** : nouveau cas dans `parse_unary` (`src/parsing/parser.d/expressions.rs`, à côté de `TokenKind::Minus` ligne 282-286) — `TokenKind::PlusPlus`/`MinusMinus` → parse récursivement un `UnaryExpr`, produit `Expr::IncDec { op: PreInc/PreDec, target: Box::new(operand), span }`.
- **Suffixe** : nouvelle alternative dans la boucle de `parse_postfix` (ligne ~300-359), à côté de `Dot`/`LParen`/`LBracket` — `TokenKind::PlusPlus`/`MinusMinus` → consomme le token, produit `Expr::IncDec { op: PostInc/PostDec, target: Box::new(expr), span }`, et **continue la boucle** (au cas où, ex. `arr[i]++` où `[i]` est déjà consommé avant d'atteindre `++` — la boucle existante gère déjà cet ordre naturellement puisque `++`/`--` s'ajoutent comme alternative de fin de chaîne).
- **Lexer** (`src/parsing/lexer.d/tokenizer.d/next_token.rs`) : lookahead sur `+`/`-` pour capter `++`/`--`, nouveaux `TokenKind::PlusPlus`/`MinusMinus` (`src/parsing/token.rs`). Vérifier l'ordre de matching pour ne pas casser la reconnaissance normale de `+`/`-` seuls.

### 3. Sema (`src/sema/typecheck.rs`, nouveau cas dans `infer_expr`)

Nouveau bras `Expr::IncDec { op, target, span }`, calqué sur le cas `Stmt::Assign` existant (lignes 786-822) pour la validation de forme, PLUS une vérification de type (contrairement à `Stmt::Assign` qui accepte n'importe quel type compatible) :

- `target` doit être `Ident` (existant, **mutable** — même erreur `InvalidAssign`/`UndefinedSymbol` que `Stmt::Assign` si non), `Field` (`infer_expr(object)`), ou `Index` (`infer_expr(object)` + `infer_expr(index)`) — sinon `SemaError::InvalidAssign { name: "cible invalide", span }`, identique au message existant.
- Type de `target` (via `infer_expr` sur le target lui-même, pas juste ses sous-expressions) doit être `Type::Int` ou `Type::Float` — sinon nouvelle erreur claire (`SemaError::TypeMismatch` ou un nouveau variant dédié `SemaError::IncDecInvalidType`, à trancher à l'implémentation) : rejette `string`/`bool`/`mixed`/classe/array/map.
- Type de retour de l'expression (`infer_expr`) : le type de `target` (`Int` ou `Float`) — identique pour les 4 formes (seule la VALEUR diffère au runtime, pas le type statique).
- Pas de nouvelle règle d'échappement (`check_escape`) : `int`/`float` sont `OwnershipClass::Unsupported` (jamais suivis par le système `scoped`/`consumed`), donc rien à faire de ce côté — confirmé par lecture de `src/sema/scope.rs::ownership_class`.

### 4. Lowering (`src/lower/expr.d/lower.rs`, nouveau cas dans `lower_expr`)

C'est la partie la plus délicate : il faut évaluer `target` UNE SEULE FOIS (charger l'ancienne valeur), calculer la nouvelle, la stocker, et retourner l'ancienne OU la nouvelle — sans jamais réévaluer deux fois une sous-expression de `target` qui aurait un effet de bord (`obj().champ++` ne doit appeler `obj()` qu'UNE fois ; `arr[calc()]++` ne doit appeler `calc()` qu'UNE fois). `lower_assign` existant ne convient pas tel quel : il attend un `Expr value` à lowered et ne retourne rien — mais sa logique de dispatch par forme de cible (`Ident`/`Field`/`Index`) est le bon patron à dupliquer/factoriser pour une nouvelle fonction dédiée, par exemple `lower_incdec(builder, op, target, span) -> Value` :

- **`Ident`** : `load_local`/`frame_vars` pour l'ancienne valeur → `ConstInt`/`ConstFloat` de `1` (type déduit du IR type de la variable) → `Add`/`Sub` IR → `store_local` (même boxing éventuel que `lower_assign::Ident`, même `free_before_reassign` — un `int`/`float` n'est jamais `scoped`/`consumed` en pratique mais garder la symétrie ne coûte rien) → retourne ancien ou nouveau selon `op`.
- **`Field`** : résoudre `class_name`/`offset` UNE fois (même logique que `lower_assign::Field`) → `lower_expr(object)` UNE fois (réutiliser la `Value` obtenue pour le `GetField` ET le `SetField` qui suivent, jamais réévaluer `object`) → `GetField` (ancienne valeur) → calcul → `SetField` (même `obj_val`) → retour ancien/nouveau.
- **`Index`** : `lower_expr(object)` et `lower_expr(index)` UNE fois chacun (réutiliser les deux `Value` pour `__array_get`/`__map_get` PUIS `__array_set`/`__map_set`) → même distinction array/map que `lower_assign::Index` → calcul → retour ancien/nouveau.

Refactor recommandé pour éviter la duplication (le projet évite le code dupliqué) : extraire de `lower_assign` une fonction interne `store_to_target(builder, target: &Expr, val: Value)` réutilisée par les DEUX call sites (`lower_assign` pour le cas normal, `lower_incdec` pour le nouveau cas) — `lower_assign` devient alors `store_to_target(builder, target, val_déjà_calculée)`. Nécessite aussi une fonction (ou la réutilisation directe de `lower_expr`) pour la partie LECTURE, déjà couverte par le comportement existant de `lower_expr` sur `Ident`/`Field`/`Index` — mais attention : `lower_expr(target)` pour un `Field`/`Index` réévalue `object`/`index` en interne ; il faut soit une variante qui accepte des `Value` déjà évaluées pour `object`/`index`, soit dupliquer le strict minimum (GetField avec un `obj_val` fourni) plutôt que d'appeler `lower_expr` deux fois sur le même sous-arbre. Ce point d'unicité d'évaluation est LE risque principal de cette implémentation — à vérifier explicitement par un test dédié (voir plus bas).

### 5. Ordre d'évaluation dans une expression composée

Comme en C : au sein d'une même expression, une variable modifiée par `++`/`--` et relue ailleurs dans la MÊME expression a un comportement qui dépend de l'ordre d'évaluation des opérandes (`i++ + i` par exemple). Ocara n'a pas besoin d'une règle plus stricte que C ici — documenter simplement l'ordre d'évaluation gauche-à-droite déjà implicite dans le lowering existant (chaque sous-expression est lowered dans l'ordre où le code source l'écrit, comme pour `Expr::Binary` aujourd'hui) plutôt que d'inventer une garantie supplémentaire.

### 6. Documentation

- `docs/EBNF.md` : nouvelles productions `UnaryExpr`/`PostfixExpr` (section expressions) — bien préciser que c'est une EXPRESSION cette fois (contrairement à l'Option A initialement envisagée), avec la sémantique pré/post explicite.
- `docs/workflow-compilation.md` : si une section décrit le pipeline lexer→parser→sema→lower avec des exemples de features, ajouter une mention (à vérifier à l'implémentation si une section s'y prête).
- Extension VSCode (`tools/`) : coloration syntaxique des nouveaux tokens `++`/`--` si la grammaire TextMate distingue les opérateurs individuellement — à vérifier à l'implémentation (désinstaller/recompiler/réinstaller par la procédure standard).
- `ocaracs`/`ocaraunit` : vérifier s'ils ont un lexer indépendant qui devrait aussi reconnaître ces tokens (peu probable qu'ils re-tokenisent le langage complet, mais à confirmer).

## Tests à prévoir

- Nouvel exemple `examples/NN_increment_decrement.oc` : les 4 formes sur `int`, sur `float`, en position de statement ET en position d'expression (`x = i++`, `arr.push(i++)`...).
- Nouveau test unitaire `examples/tests/NN_increment_decrementTest.oc` couvrant :
  - Sémantique pré vs post distincte : `var i:int = 5; assertEquals(5, i++); assertEquals(6, i)` / `var j:int = 5; assertEquals(6, ++j); assertEquals(6, j)` (et l'équivalent pour `--`).
  - `float` : mêmes vérifications avec un pas de `1.0`.
  - Cible `Field` (`self.compteur++`) et `Index` (`arr[0]++`, `map["clé"]++`) — avec un test qui prouve l'unicité d'évaluation (ex. une méthode qui incrémente un compteur externe à chaque appel comme index : `arr[bump()]++` puis vérifier que le compteur externe n'a avancé qu'une fois).
  - Un test `--check`-only (voir `examples/21_errors.oc` pour le patron) confirmant qu'une cible invalide (`5++`, `(x + 1)++`, ou un type incompatible comme `string`/`bool`) est bien rejetée à la compilation.
  - Un test confirmant que `- -5` (avec espace) reste bien une double négation valant `5` — non-régression explicite sur l'unaire `-` existant.

## Priorité / Complexité

**Structurel** (Option B) — nouveau nœud AST, grammaire à deux niveaux (préfixe et suffixe), validation sémantique de type ET de forme, lowering avec garantie d'unicité d'évaluation des sous-expressions. Non-bloquant pour la fiabilité du langage (confort d'écriture), mais à ne pas improviser — bons tests de non-régression nécessaires avant et après, notamment sur l'unaire `-` existant et sur toute expression composée utilisant déjà `+`/`-` de façon adjacente.
