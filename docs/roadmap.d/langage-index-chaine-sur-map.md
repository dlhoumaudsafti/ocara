# `expr[i][clé]` (indexation chaînée array-puis-map) retourne `null` au lieu de la vraie valeur

Vérifié :
- Hypothèse du ticket confirmée : `is_map_target` (`src/lower/expr.d/helpers.rs`) ne gérait que `Expr::Ident`/`Expr::Field` comme objet d'un `Expr::Index`, jamais un `Expr::Index` imbriqué (tombait dans le `_ => false`, dispatchait vers `__array_get` au lieu de `__map_get`).
- Corrigé par une résolution **récursive** du type d'une indexation chaînée, `elem_type_after_index(builder, expr) -> Option<Type>` (nouvelle fonction, `src/lower/expr.d/helpers.rs`) : pour `Expr::Ident`, lit `builder.elem_ast_types` (inchangé) ; pour `Expr::Field`, résout la classe du récepteur puis lit `module.class_field_types` (inchangé) ; pour `Expr::Index { object, .. }` (le cas manquant), s'appelle récursivement sur `object` puis pèle **une** couche de conteneur (`container_elem_type` : `Type::Array`/`Type::Map`/`Type::Union` dépliés). `is_map_target` route désormais son cas `Expr::Index` par cette fonction — supporte donc une profondeur de chaînage **arbitraire** (`a[0][1]["x"]["y"]`...), pas seulement les deux niveaux du repro original, par composition de la récursion plutôt que par un cas à profondeur fixe.
- Point 3 de la demande initiale (l'autre sens de nesting, `map<string, map<...>>`, et la profondeur > 2) répondu : couvert par `container_elem_type` (une seule fonction pour les deux sens de nesting, pas un cas séparé par direction) et testé explicitement (voir tests ci-dessous).
- Fonction partagée par la lecture (`src/lower/expr.d/lower.rs`, `Expr::Index`) ET l'écriture (`src/lower/stmt.d/statements.d/assignments.rs`, deux points d'appel) — un seul correctif couvre les deux chemins, vérifié par `chainedIndexAssignmentTest`.
- **7 tests unitaires Rust** ajoutés dans `src/lower/expr.d/tests.rs` (partagé avec la fiche jumelle `qualite-parite-sucre-statique-param-types.md`) : non-régression 1 niveau (`Ident` inchangé), profondeur 2 (repro exact), profondeur 3 (deux couches array pelées), profondeur 5 (aucune limite codée en dur), non-régression arrays purs (jamais classés map), union `map<K,V>|null` dépliée, expression inconnue → `None`/`false` sans panic. Un off-by-one dans la construction manuelle du test de profondeur 5 (une couche `Type::Array` de trop autour de la map, `elem_ast_types` représentant déjà un niveau d'indexation consommé) a été trouvé et corrigé pendant l'écriture — bug du test, pas de l'implémentation.
- **9 assertions** dans un nouvel exemple `examples/tests/50_chained_index_on_mapTest.oc` : profondeur 2/3/4 en lecture, écriture chaînée, chaînage à travers un champ de classe (`self.champ[i][clé]`, via `class_field_types`), map-de-map, non-régression arrays purs.
- `cargo test -p ocara` : 74 passed (dont les 7 nouveaux). `cargo test -p ocara_runtime` : 57 passed + 7 `#[ignore]` (inchangé, sans rapport avec ce fix).
- `make build` (les 4 crates) + `RUSTFLAGS="-D warnings"` : 0 warning.
- `make regression` (cache vidé) : 668 PASS / 0 FAIL, 0 ERREUR(S) — aucune régression (9 PASS de plus qu'avant ce ticket, exactement les 9 nouvelles assertions).
- `docs/EBNF.md` : non touché — correctif de compilateur pur, aucun changement de syntaxe.

## Constat

Trouvé par accident en vérifiant le ticket [stdlib-mysql-requetes-parametrees-transactions](stdlib-mysql-requetes-parametrees-transactions.md) — **sans rapport avec MySQL**, reproduit à l'identique avec SQLite (non touché par ce ticket) :

```ocara
const arr:array<map<string, mixed>> = db.query("SELECT * FROM t")
IO::writeln(`${arr[0]["name"]}`)   // affiche "null" — FAUX
```
```ocara
const first:map<string, mixed> = arr[0]
IO::writeln(`${first["name"]}`)   // affiche "Alice" — correct, via une variable intermédiaire
```

Reproduction minimale avec `examples/tests/` (voir `docs/roadmap.d/stdlib-sqlite-requetes-parametrees-transactions.md`, aucun rapport avec ce ticket-ci, juste utilisé comme terrain de repro) :

```ocara
const arr:array<map<string, mixed>> = db.query("SELECT * FROM t")   // 1 ligne, colonnes name/age
IO::writeln(`chained: name=${arr[0]["name"]} age=${arr[0]["age"]}`)   // null / null
const first:map<string, mixed> = arr[0]
IO::writeln(`via var: name=${first["name"]} age=${first["age"]}`)     // Alice / 30 (correct)
```

Seule différence entre les deux : une indexation **directe et chaînée** (`arr[0]["name"]`, deux `Expr::Index` imbriqués) contre la même valeur lue d'abord dans une variable puis indexée une seule fois.

## Hypothèse de cause (non vérifiée par du débogage pas à pas, mais cohérente avec le bug jumeau déjà corrigé dans ce même ticket)

`src/lower/expr.d/lower.rs`, `Expr::Index { object, index, .. }` décide `__map_get` vs `__array_get` via `is_map_target(builder, object)` (`src/lower/expr.d/helpers.rs:218`) :

```rust
pub fn is_map_target(builder: &LowerBuilder, object: &Expr) -> bool {
    match object {
        Expr::Ident(name, _) => builder.map_vars.contains(name.as_str()),
        Expr::Field { object: inner, field, .. } => { ... }
        _ => false,
    }
}
```

Pour `arr[0]["name"]`, l'`object` de l'`Expr::Index` EXTÉRIEUR est `arr[0]` — lui-même un `Expr::Index`, pas un `Expr::Ident`/`Expr::Field`. Il tombe donc dans le `_ => false` : `is_map_target` répond "non", `Expr::Index` dispatche vers `__array_get` au lieu de `__map_get`, sur un pointeur qui est en réalité une map — lecture de mémoire au mauvais format, valeur `0`/`null` en pratique plutôt qu'un crash (cohérent avec ce qui a été observé).

C'est très probablement le même type de lacune que le bug déjà corrigé dans ce ticket (`map_value_type` ne dépliait pas `Type::Union` — voir `stdlib-mysql-requetes-parametrees-transactions.md`) : `is_map_target` couvre `Ident`/`Field` mais pas `Index` comme objet d'un `Index` englobant. Si l'élément d'un `array<map<K,V>>` (ou la valeur d'une `map<K, map<K2,V2>>`) doit être reconnu comme une map pour permettre l'indexation chaînée, `is_map_target` doit soit résoudre le type statique de l'expression `object` de façon générale (pas seulement pour `Ident`/`Field`), soit gérer explicitement le cas `Expr::Index` en consultant `builder.elem_ast_types`/`elem_types` de la variable indexée à l'intérieur.

## Ce qui est demandé

1. Confirmer l'hypothèse ci-dessus par un test unitaire ciblé sur `is_map_target`/`Expr::Index` (voir `src/lower/expr.d/helpers.rs`), dans l'esprit de `map_value_type` (fiche jumelle).
2. Étendre `is_map_target` (ou son appelant) pour reconnaître `Expr::Index { object, .. }` quand le TYPE ÉLÉMENT de `object` est lui-même une map — `builder.elem_ast_types`/`elem_types` (déjà peuplés pour les variables `array<map<...>>`/`map<K, map<...>>`, voir `src/lower/stmt.d/statements.d/variables.rs`) devraient suffire à répondre sans réécriture profonde.
3. Vérifier si le même défaut existe dans l'autre sens (map contenant des arrays, `m["clé"][0]`) et pour une profondeur > 2 (`arr[0]["clé"][1]`).
4. Ajouter un exemple de régression `.oc` dédié (indexation chaînée sur `array<map<...>>` ET `map<..., array<...>>`) — aucun exemple existant du corpus ne semble exercer ce chemin, ce qui explique qu'il soit resté invisible jusqu'ici.

## Priorité / Complexité

**Terminé.** Était Priorité Basse, Complexité Légère à Structurel selon ce que révélerait le point 1 — confirmé Légère : une seule fonction récursive (`elem_type_after_index`) a suffi, sans réécriture profonde, en généralisant le mécanisme existant plutôt qu'en ajoutant un cas spécial à profondeur fixe.

## Fichiers clés

`src/lower/expr.d/lower.rs` (`Expr::Index`), `src/lower/expr.d/helpers.rs` (`is_map_target`), `src/lower/stmt.d/statements.d/variables.rs` (`elem_types`/`elem_ast_types`, déjà peuplés pour les variables `array<map<...>>`).
