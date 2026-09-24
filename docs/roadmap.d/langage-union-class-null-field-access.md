# Un accès de champ sur `Classe|null` (ou la variable d'un `for` sur `array<Classe>`) retourne toujours le PREMIER champ, quel que soit celui demandé

Vérifié :
- Hypothèse initiale confirmée puis élargie : la cause n'est pas un dépliage d'union manquant dans la résolution d'offset de `Expr::Field` (`src/lower/expr.d/lower.rs`), mais l'absence d'entrée dans `builder.var_class` — la table qui permet à `Expr::Field`/`Expr::Field` en écriture (`src/lower/stmt.d/statements.d/assignments.rs`)/`Expr::IncDec` de résoudre la classe d'un `Expr::Ident` — pour CHAQUE site de binding qui déclare une variable/un paramètre à partir d'un type `Classe|null`, ou d'un élément de conteneur (`array<Classe>`, `map<K,Classe>`) qui est lui-même une classe.
- Root cause confirmée par reproduction ciblée (`const f:Foo|null = Repo::find()` compile ET s'exécute, mais `f.reference`/`f.brand`/`f.model` valent tous `"99"`, la valeur de `f.id`) : `lower_const` (`src/lower/stmt.d/statements.d/variables.rs`) n'avait JAMAIS le cas `Type::Union` ajouté à `lower_var` — donc `f` n'a aucune entrée `var_class`, `Expr::Field` résout `class_name = None`, ce qui retombe sur `offset = 0` (voir `field_offset`, `src/lower/expr.d/helpers.rs`) pour N'IMPORTE QUEL champ.
- **Second déclencheur, sans union**, trouvé en élargissant l'investigation (même symptôme, cause apparentée) : `for it in items` où `items:array<Foo>` — `lower_for_in` (`src/lower/stmt.d/statements.d/loops.rs`) ne gérait QUE le cas où l'élément itéré est un `map` (`array<map<K,V>>`), jamais `array<Classe>` : la variable de boucle n'avait alors AUCUNE entrée `var_class`, quel que soit le champ accédé dans le corps de la boucle.
- **Troisième déclencheur, même famille**, trouvé en creusant `loops.rs` : `for k => v in m` où `m:map<K,Classe>` — `lower_for_map` n'enregistrait AUCUNE métadonnée de classe pour la variable VALEUR, quel que soit le type de valeur de la map (contrairement à `lower_for_in`, qui gérait au moins le cas map).
- **Quatrième déclencheur, même famille**, trouvé en généralisant : un paramètre de fonction/méthode ou de closure nameless typé `Classe|null` (`src/lower/builder.d/functions.rs`, `src/lower/expr.d/nameless.rs`) — même absence de dépliage `Type::Union` que `lower_const`.
- Écriture (`f.champ = valeur`) et incrément/décrément (`f.champ++`) confirmés atteints par LE MÊME bug, PAS une cause séparée : `lower_assign`/`lower_incdec` (`src/lower/stmt.d/statements.d/assignments.rs`) résolvent la classe d'un `Expr::Ident` en lisant `builder.var_class` exactement comme la lecture — une fois `var_class` correctement peuplé à la déclaration/au binding, lecture ET écriture sont corrigées par le MÊME correctif, sans toucher `assignments.rs`. Vérifié par reproduction : `const f:Foo|null = ...; f.reference = "ZZZ"` corrompait `id`/`brand`/`model` (tous devenaient une variante de `"ZZZ"`) avant le correctif, écrit correctement uniquement `reference` après.
- Corrigé par un point de résolution partagé plutôt que 5 correctifs isolés : `union_named_class`/`resolved_named_class` (nouvelles fonctions pures, `src/parsing/ast.d/types.rs` — placées là, pas dans un seul module `lower::*`, car leurs appelants sont dispersés dans les trois sous-arbres du lowering, `builder.d`/`expr.d`/`stmt.d`), et `register_var_class` (nouvelle fonction, `src/lower/stmt.d/statements.d/variables.rs`, partagée par `lower_var`/`lower_const` — élimine au passage deux AUTRES divergences déjà présentes entre les deux, `Type::Map`/`Type::Function` jamais gérés par `lower_const`).
- **18 tests unitaires Rust** ajoutés : 5 dans `src/parsing/ast.d/types.rs` (`union_named_class`/`resolved_named_class`, dont l'ordre des variantes et l'absence de variante `Named`), 7 dans `src/lower/stmt.d/statements.d/variables.rs` (`register_var_class`, dont Union/Function/primitif-non-régression), 6 dans `src/lower/stmt.d/statements.d/loops.rs` (`lower_for_in`/`lower_for_map`, dont non-régression du cas map déjà géré).
- **27 assertions** dans un nouvel exemple `examples/tests/56_union_class_null_field_accessTest.oc` : le repro exact (`const`), non-régression `var`, écriture de champ via `const` narrowed, champ d'un champ (`p.address.city`) à travers un narrowing union, `for it in array<Classe>`, `for k => v in map<K,Classe>`, paramètre de fonction `Classe|null`, non-régression classe concrète (jamais union).
- `cargo test -p ocara` : 128 passed (dont les 18 nouveaux). `cargo test -p ocara_runtime` : 57 passed + 7 `#[ignore]` (inchangé, sans rapport avec ce fix).
- `make build` (les 4 crates) + `RUSTFLAGS="-D warnings"` : 0 warning.
- `./ci/regression.sh` : tous les tests noir-boîte passent. `./ci/unittests.sh examples/project/tests` : 50 PASS / 0 FAIL. `./ci/unittests.sh examples/tests` : 714 PASS / 0 FAIL, 0 ERREUR(S) — 687 PASS avant ce ticket (aucune régression), +27 PASS exactement les nouvelles assertions.
- `docs/EBNF.md` : non touché — correctif de compilateur pur, aucun changement de syntaxe ; le pattern `Classe|null` documenté (retour "peut échouer") fonctionne maintenant réellement comme documenté.

## Constat

Repro minimal (aucune branche nécessaire — un seul `return use Foo(...)` avec un type de retour déclaré `Foo|null` suffit) :

```ocara
class Foo {
    public property id:int
    public property reference:string
    public property brand:string
    public property model:string
    init(id:int, reference:string, brand:string, model:string) {
        self.id = id
        self.reference = reference
        self.brand = brand
        self.model = model
    }
}

class Repo {
    public static method find(): Foo|null {
        return use Foo(99, "AAA", "BBB", "CCC")
    }
}

function main(): int {
    const f:Foo|null = Repo::find()
    if f is null { return 1 }
    IO::writeln(f.reference)   // affiche "99" — FAUX, devrait afficher "AAA"
    IO::writeln(f.brand)       // affiche "99" — FAUX, devrait afficher "BBB"
    return 0
}
```

Contournement qui fonctionnait déjà (et qui a orienté l'investigation vers une résolution de classe manquante plutôt qu'un bug d'offset lui-même) : réassigner vers une variable de type CONCRET juste après le narrowing —

```ocara
const g:Foo = f
IO::writeln(g.reference)   // "AAA" — correct
```

Second repro, sans aucun union, trouvé en élargissant l'investigation à la demande :

```ocara
var items:array<Foo> = []
items.push(use Foo(1, "AAA", "BBB", "CCC"))
for it in items {
    IO::writeln(it.reference)   // affiche "1" (= it.id) — FAUX, devrait afficher "AAA"
}
```

## Cause

`Expr::Field` (lecture, `src/lower/expr.d/lower.rs`) et son équivalent en écriture/incrément (`src/lower/stmt.d/statements.d/assignments.rs`) résolvent la classe d'un récepteur `Expr::Ident(name)` en lisant `builder.var_class.get(name)`. Si cette table n'a aucune entrée pour `name`, `class_name` vaut `None`, et l'offset du champ retombe sur `0` (`field_offset`) quel que soit le champ demandé — silencieusement toujours la valeur du PREMIER champ déclaré, jamais une erreur de compilation ni un crash.

`var_class` est peuplée au moment où chaque variable/paramètre est LIÉE à un type, par plusieurs sites indépendants qui dupliquaient chacun leur propre logique de dérivation à partir du `Type` déclaré/résolu :
- `lower_var`/`lower_const` (`src/lower/stmt.d/statements.d/variables.rs`) pour `var`/`const` ;
- `lower_for_in`/`lower_for_map` (`src/lower/stmt.d/statements.d/loops.rs`) pour la variable de boucle ;
- l'enregistrement des paramètres de fonction/méthode (`src/lower/builder.d/functions.rs`) ;
- l'enregistrement des paramètres de closure nameless (`src/lower/expr.d/nameless.rs`).

Seul `lower_var` dépliait un `Type::Union` (`Classe|null`) pour y trouver la classe — ajouté à un moment de l'historique du projet sans jamais être répercuté sur les quatre autres sites listés ci-dessus, qui ne géraient que `Type::Named` direct (ou, pour les deux fonctions de `loops.rs`, ne géraient même pas du tout le cas classe : seul l'élément `map` y était reconnu). Exactement la même classe de risque que d'autres bugs déjà documentés dans ce projet (voir `qualite-parite-sucre-statique-param-types.md`) : un correctif ajouté à une copie d'une logique dupliquée, jamais répercuté sur les autres.

## Correctif

Un point de résolution partagé plutôt que N correctifs isolés :

- `union_named_class(ty: &Type) -> Option<String>` (`src/parsing/ast.d/types.rs`) : premier variant `Type::Named` d'un `Type::Union` — un seul niveau de dépliage suffit, un `Type::Union` ne pouvant pas être imbriqué dans un autre (voir `Parser::parse_type`).
- `resolved_named_class(ty: &Type) -> Option<String>` (même fichier) : `Type::Named` direct OU `union_named_class` — la question posée par `lower_for_in`/`lower_for_map` ("quelle classe pour cet élément de conteneur ?").
- `register_var_class(builder, name, ty)` (`src/lower/stmt.d/statements.d/variables.rs`) : factorise TOUTE la dérivation `var_class`/`func_vars`/`func_ret_types` à partir d'un type déclaré, appelée par `lower_var` ET `lower_const` — élimine au passage deux divergences supplémentaires (`Type::Map`→"Map" et `Type::Function` jamais gérés par `lower_const`).
- `lower_for_in`/`lower_for_map` (`loops.rs`), l'enregistrement de paramètre (`functions.rs`), et l'enregistrement de paramètre de closure (`nameless.rs`) appellent désormais `resolved_named_class`/`union_named_class` au lieu de ne reconnaître que `Type::Named` direct (ou rien du tout, pour l'élément classe d'un `array<Classe>` dans `loops.rs`).

Placé dans `src/parsing/ast.d/types.rs` (plutôt que dans un seul des modules `lower::*`) précisément parce que ses appelants sont dispersés dans les trois sous-arbres indépendants du lowering (`builder.d`, `expr.d`, `stmt.d`) qui ne peuvent pas s'importer librement entre eux (`statements_impl` est un module privé) — le placer au niveau de `Type` lui-même évite d'avoir à replumber les `pub use` de `statements.rs` et rend la fonction trivialement accessible partout où `Type` l'est déjà.

## Priorité / Complexité

**Terminé.** Était Priorité Haute (correction silencieuse de données sur le pattern `T|null` officiellement documenté) — confirmé Légère : aucune réécriture d'architecture, une paire de fonctions pures + une factorisation dans un fichier déjà existant ont suffi.

## Fichiers clés

`src/parsing/ast.d/types.rs` (`union_named_class`, `resolved_named_class`), `src/lower/stmt.d/statements.d/variables.rs` (`register_var_class`, `lower_var`, `lower_const`), `src/lower/stmt.d/statements.d/loops.rs` (`lower_for_in`, `lower_for_map`), `src/lower/builder.d/functions.rs` (enregistrement des paramètres), `src/lower/expr.d/nameless.rs` (paramètres de closure), `src/lower/expr.d/lower.rs` (`Expr::Field`, jamais modifié — bénéficie du correctif sans y toucher), `src/lower/stmt.d/statements.d/assignments.rs` (écriture/incrément de champ, jamais modifié, même raison).
