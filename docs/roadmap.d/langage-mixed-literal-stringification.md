# Les littéraux `float`/`bool` dans un conteneur `array`/`map` étaient stockés comme des strings

## ✅ Corrigé

## Constat (avant correctif)

Découvert en travaillant sur le support YAML (`docs/roadmap.d/builtins-yaml.md`). Dans le lowering d'un littéral `array`/`map` (`src/lower/expr.d/lower.rs`), tout élément/valeur de type `float` ou `bool` était converti en string (`__str_from_float`/`__str_from_bool`) **avant** d'être stocké — **inconditionnellement**, quel que soit le type déclaré du conteneur (pas seulement `mixed` comme le titre initial de cette fiche le laissait penser).

```ocara
const data:map<string, mixed> = { 'pi': 3.14159, 'count': 42, 'active': true }
IO::writeln(JSON::encode(data))
// {"active":"true","count":42,"pi":"3.14159"}
//           ^^^^^^              ^^^^^^^^^^^
//  bool ET float rendus comme des chaînes JSON, alors que count (int) reste un vrai nombre
```

**Découverte plus large en creusant** : `array<float>`/`array<bool>` (des tableaux **non-mixed**, homogènes) étaient **eux aussi** entièrement cassés par ce même code, sans rapport avec `mixed` — `var arr:array<float> = [1.5, 2.5]; IO::writeln(arr[0])` affichait un nombre dénormalisé proche de zéro au lieu de `1.5` : la lecture (`arr[0]`) renvoie le pointeur de la string stockée, réinterprété comme des bits flottants.

## Correctif

Deux comportements distincts selon que le type d'élément déclaré est connu et concret, ou `mixed`/inconnu à cet endroit (`src/lower/expr.d/lower.rs`, nouvelles fonctions `lower_array_literal`/`lower_map_literal` + `LiteralElemKind`) :
- **`LiteralElemKind::Concrete`** (type d'élément déclaré connu et différent de `mixed`, ex. `array<float>`) : élément stocké **brut**, sans la moindre conversion — comme `int`, qui n'a jamais été boxé nulle part dans ce compilateur. Utilisé par `lower_var`/`lower_const` (`src/lower/stmt.d/statements.d/variables.rs`) quand le littéral est directement affecté à une variable dont le type est explicitement connu.
- **`LiteralElemKind::Mixed`** (type déclaré `mixed`, ou type de destination inconnu à cet endroit — littéral imbriqué, argument, valeur de retour) : élément **boxé** (`__box_float`/`__box_bool`, la représentation "mixed" déjà utilisée par `box_for_any`/`__dyn_add`), plus jamais stringifié — c'est le choix par défaut, utilisé par le dispatch générique de `lower_expr` pour `Expr::Array`/`Expr::Map` (nested/argument/retour, où le type de destination n'est pas accessible ici).

**Consommateurs mis à jour en conséquence** (le boxing ne sert à rien si personne côté runtime ne sait le déballer) :
- `runtime/src/lib.rs` (`value_to_json`, utilisé par `JSON::encode`) : ne vérifiait JAMAIS `is_float_box`, et confondait un bool boxé avec un entier (le boxing produit un grand pointeur, jamais littéralement `0`/`1`). Corrigé : `is_float_box`/`is_bool_box` vérifiés et déballés en priorité.
- `runtime/src/yaml.rs` (`value_to_yaml`, `YAML::encode`) : gérait déjà `is_float` (correctif antérieur, voir `builtins-yaml.md`) mais pas le bool boxé — corrigé de la même façon (vérification du tag AVANT tout déballage, jamais un `__unbox_bool` avancé à l'aveugle sur un `0`/`1` brut qui n'est pas un pointeur).
- `runtime/src/typecheck.rs` (`__is_bool`, narrowing `x is bool`) : ne reconnaissait qu'un bool JAMAIS boxé (`val == 0/1`) — reconnaît maintenant aussi le tag de boxing, cohérent avec `__is_float` qui le fait déjà.

Vérifié : la reproduction exacte du constat initial produit maintenant `{"active":true,"count":42,"pi":3.14159}` (JSON::encode) et l'équivalent correct en YAML ; `var arr:array<float> = [1.5, 2.5]` se lit et s'additionne désormais correctement. Deux nouvelles assertions ajoutées à `examples/tests/16_typesTest.oc` (`array<bool>`/`array<float>` littéraux déjà déclarés dans ce test mais jamais vérifiés avec une égalité précise — `assertTrue` ne fait que vérifier `!= 0`, ce qui passait à tort même avec l'ancien bug ; `array<float>` n'avait aucune couverture nulle part dans le dépôt). `make regression` : 388 PASS / 0 FAIL (2 de plus), 49 PASS / 0 FAIL / 0 ERREUR côté projet — aucune régression.

## Découvertes annexes non corrigées (pré-existantes, sans rapport direct)

- **Le passage d'une valeur `float`/`bool` en argument d'un paramètre `mixed` ne la boxe jamais** (`function f(x:mixed)` appelée avec `f(true)` ou `f(3.14)`) — seuls `box_for_any` (affectation `var`/`const`/assignation) et maintenant les littéraux `array`/`map` boxent correctement. Un `bool`/`float` qui transite uniquement par un argument de fonction reste donc ambigu au runtime. Plus large que cette fiche (toucherait le passage d'arguments de tout appel de fonction, pas seulement les littéraux) — non traité ici.
- **`value_to_json`/`value_to_yaml` confondent un entier BRUT valant exactement `0` ou `1` avec un booléen** (`array<int> = [1, 2, 3]` encodé en JSON affiche `[true,2,3]`) — limitation déjà présente et documentée en commentaire (`__is_bool`) avant ce correctif, pas introduite ici. Non corrigée : la retirer casserait le seul cas où un bool passé en argument `mixed` (voir ci-dessus, jamais boxé) est aujourd'hui détecté, même imparfaitement.
- **`IO::writeln(JSON::encode(x))` (appel direct, sans variable intermédiaire) affiche un nombre incohérent** au lieu de la string JSON — `expr_ir_type` ne reconnaît pas `JSON_encode`/les méthodes d'instance (`x.encode()`) comme retournant `Ptr`, contrairement à `String_*`/`Array_join`/etc. déjà listés. Confirmé aussi pour un `array<int>` simple, sans rapport avec `mixed` : `var s:string = JSON::encode(arr); IO::writeln(s)` fonctionne, `IO::writeln(JSON::encode(arr))` directement non. Non corrigé ici (chantier séparé, dispatch de type des appels, pas le stockage des littéraux).

## Fichiers clés

`src/lower/expr.d/lower.rs` (`lower_array_literal`, `lower_map_literal`, `LiteralElemKind`, `box_for_dyn_arith` réutilisé), `src/lower/stmt.d/statements.d/variables.rs` (`lower_literal_or_expr`), `runtime/src/lib.rs` (`value_to_json`), `runtime/src/yaml.rs` (`value_to_yaml`), `runtime/src/typecheck.rs` (`__is_bool`), `examples/tests/16_typesTest.oc`, `examples/16_types.oc`.
