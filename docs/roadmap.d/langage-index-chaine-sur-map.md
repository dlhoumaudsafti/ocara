# `expr[i][clé]` (indexation chaînée array-puis-map) retourne `null` au lieu de la vraie valeur

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

**Priorité Basse** — contournement simple et déjà documenté (variable intermédiaire), donc pas bloquant, mais c'est un vrai bug de correction silencieuse (aucune erreur, aucun warning, juste une valeur fausse) sur un pattern de code par ailleurs tout à fait naturel (`rows[0]["champ"]`). **Complexité Légère à Structurel selon ce que révèle le point 1** — probablement une extension ciblée de `is_map_target`, mais à confirmer avant de s'engager.

## Fichiers clés

`src/lower/expr.d/lower.rs` (`Expr::Index`), `src/lower/expr.d/helpers.rs` (`is_map_target`), `src/lower/stmt.d/statements.d/variables.rs` (`elem_types`/`elem_ast_types`, déjà peuplés pour les variables `array<map<...>>`).
