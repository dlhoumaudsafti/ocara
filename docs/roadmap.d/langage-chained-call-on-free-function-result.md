# Appel chaîné cassé sur le résultat d'une fonction LIBRE — `maFonction(...).methode()` retourne `null` silencieusement

Vérifié :
- **Root cause confirmée par `ocara build --dump` (HIR), pas seulement supposée par analogie** — exactement la méthode demandée : avant correctif, `pickCircle(0).shapeName()` émettait `Call { dest: Some(v2), func: "_method_shapeName", args: [v1], ret_ty: Ptr }` — aucun préfixe de classe, un symbole qui n'existe nulle part, silencieusement sans effet au codegen (`IO::writeln` affichait `"null"`, jamais une erreur de compilation). L'hypothèse du ticket original était correcte dans son principe (même famille que `use Classe(...).methode()`/`HTTPRequest::get(...).methode()`, même famille de points de résolution) mais incomplète dans le détail : la fonction connaît bien son type de retour DÉCLARÉ (`fn_ret_types`), mais celui-ci est un `IrType` RÉDUIT (`Ptr` pour toute classe/string/array/map, indistinguables) — il manquait un mapping séparé nom-de-fonction → nom-de-CLASSE CONCRET, qui n'existait nulle part avant ce ticket.
- **Corrigé par un nouveau champ `IrModule::func_ret_class`** (`src/ir/module.rs`), peuplé une fois dans `lower_program` (`src/lower/builder.d/program.rs`) pour chaque fonction libre dont le retour est soit un `Type::Named` (classe utilisateur), soit une famille builtin `string`/`array`/`map` (même convention que `resolve_chained_field_class` pour un champ) — consulté dans les DEUX points de résolution identiques au ticket original : `src/lower/expr.d/lower.rs` (lowering réel du site d'appel) et `src/lower/expr.d/typeinfer.rs` (inférence de type pour les décisions de boxing en aval). Placé sur `IrModule` (accessible partout via `builder.module.func_ret_class`) plutôt que threadé à travers tous les constructeurs de `LowerBuilder` (comme `fn_ret_types`) — plus simple, même patron déjà utilisé pour `class_ids`/`is_check_candidates`/`class_parents`, des données globales précalculées une fois.
- **Cas généraux vérifiés, pas seulement l'exemple du ticket** (demande explicite) : classe utilisateur avec méthode retournant `string` (cas exact du ticket) ET `float` (deuxième méthode, même classe) ; fonction libre appelée deux fois indépendamment (pas de pollution d'état) ; **familles de récepteur différentes** — une fonction libre retournant `array<T>` (`.len()` chaîné), `map<K,V>` (`.has()` chaîné), `string` (`.upper()` chaîné sur un littéral builtin, pas une classe utilisateur). Ce dernier point a nécessité d'étendre `func_ret_class` au-delà des seules classes utilisateur (`Type::Array`/`Type::Map`/`Type::String` en plus de `Type::Named`) — sans cela, `makeNames().len()` échouait exactement comme `pickCircle(0).shapeName()` avant correctif.
- **Confirmé, comme demandé** : ce ticket ne nécessite AUCUN changement d'EBNF — aucune nouvelle syntaxe, un pur correctif de lowering (une table de métadonnées manquante). `docs/diagnostics.md` non plus : aucun nouveau diagnostic, le programme compilait déjà sans erreur (le bug était un résultat FAUX SILENCIEUX, jamais un rejet à la compilation).
- **Limitation DISTINCTE découverte en testant les cas généraux, délibérément NON corrigée ici** : un appel chaîné à partir de 3 niveaux (`a.b().c().d()`) reste cassé — confirmé GÉNÉRAL et préexistant, reproduit avec une chaîne de méthodes d'instance PURE, sans la moindre fonction libre en jeu (`w.getCircle().shapeName().upper()`). Cause : toute la famille de résolution "classe du récepteur" ne recurse qu'un seul niveau ; une vraie correction demande un refactor en fonction récursive partagée, pas un cas de plus dans les `match` existants — documenté séparément dans [langage-chained-call-depth-limit](langage-chained-call-depth-limit.md), ajouté à `docs/roadmap.md`.
- **4 nouveaux tests Rust** (`src/lower/expr.d/tests.rs`) : `func_ret_class` connaît le nom de classe utilisateur d'une fonction libre ; `func_ret_class` connaît les familles builtin `Array`/`Map`/`String` ; le site d'appel réel (`Inst::Call.func`, pipeline complet `lower_program`) mangle bien vers `"Circle_shapeName"` pour le cas exact du ticket ; non-régression — une fonction retournant un type primitif (`int`) n'apparaît jamais dans `func_ret_class`. `cargo test --bin ocara --release` : **179 passed, 0 failed** (175 avant ce ticket + 4).
- **Exemple de régression bout-en-bout** : `examples/63_chained_call_on_free_function_result.oc` (illustratif) et `examples/tests/63_chained_call_on_free_function_resultTest.oc` (8 assertions : cas exact, deuxième méthode, double invocation indépendante, array/map/string). `examples/advanced/mini_project`/`mini_project_hexa` explicitement PAS touchés.
- `make build` (les 4 crates) + `RUSTFLAGS="-D warnings"` : 0 warning. `./ci/regression.sh` : tous verts. `./ci/unittests.sh examples/project/tests` : 50 PASS / 0 FAIL (inchangé). `./ci/unittests.sh examples/tests` : **812 PASS / 0 FAIL, 0 ERREUR(S)** (804 avant ce ticket, +8 nouvelles assertions).

## Constat (ticket original)

Même famille de bug que [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md)
(déjà clos) — un récepteur chaîné qui n'est PAS une variable nommée fait
échouer la résolution de sa classe — mais un déclencheur différent, non
couvert par ce correctif : le résultat d'un appel de **fonction libre**
utilisé directement comme récepteur d'un appel de méthode chaîné.

```ocara
function pickCircle(kind:int): Circle {
    return use Circle(2.0)
}
function main(): int {
    IO::writeln(pickCircle(0).shapeName())  // affichait "null", pas "circle"
    return 0
}
```

Trouvé en marge de [langage-interface-instance-dispatch-segfault](langage-interface-instance-dispatch-segfault.md).
Contournement disponible avant correctif : lier le résultat à une variable
d'abord (`var c:Circle = pickCircle(0); c.shapeName()`).

## Priorité / Complexité

**Terminé.** Complexité confirmée Légère, comme anticipé par analogie avec
le correctif déjà fait — un cas de plus aux mêmes points de résolution déjà
identifiés, pas un mécanisme nouveau. La seule extension au-delà de
l'hypothèse initiale a été la couverture des familles builtin
(`array`/`map`/`string`), demandée explicitement pour ne pas restreindre le
correctif à un seul type de retour.

## Fichiers clés

`src/ir/module.rs` (`IrModule::func_ret_class`, nouveau champ),
`src/lower/builder.d/program.rs` (population), `src/lower/expr.d/lower.rs`
(nouveau bras `Expr::Call { callee: Expr::Ident }` dans la résolution de
classe du récepteur chaîné), `src/lower/expr.d/typeinfer.rs` (même ajout,
miroir), `src/lower/expr.d/tests.rs` (4 nouveaux tests),
`examples/63_chained_call_on_free_function_result.oc`,
`examples/tests/63_chained_call_on_free_function_resultTest.oc`.
