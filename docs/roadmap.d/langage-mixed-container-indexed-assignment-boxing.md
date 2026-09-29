# `conteneur[clé] = valeur` sur un `map<string,mixed>`/`array<mixed>` ne boxe jamais `valeur` — SIGSEGV pour un float

Vérifié :
- Reproduit à l'identique le repro fourni (`gdb -q -batch -ex run -ex bt`) : `SIGSEGV` dans `__mixed_to_float`, en lisant `m["c"]` juste après `m["c"] = 3.14` (`m:map<string, mixed>`), sans SQLite, sans classe — uniquement une affectation indexée Ocara-level.
- Root cause confirmée, même FAMILLE que le correctif SQLite qui précède ce ticket (`stdlib-sqlite-integer-column-boxing.md`) mais sur un site totalement différent : `lower_assign` (`src/lower/stmt.d/statements.d/assignments.rs`), branche `Expr::Index` (affectation par indexation, `objet[index] = valeur`), transmettait `val` (la valeur RHS déjà lowered) directement à `__map_set`/`__array_set`, sans JAMAIS appeler `box_for_any` — contrairement à la branche `Expr::Ident` (affectation de variable simple, `x = valeur`) juste au-dessus dans la même fonction, qui appelle déjà `box_for_any` depuis le début. Un float littéral (`3.14`) lowered en valeur IR `F64` (registre flottant Cranelift) était donc transmis tel quel comme argument d'un appel qui attend un `i64` "mixed" auto-décrit — un bit-pattern IEEE-754 qui ressemble à un pointeur heap valide une fois réinterprété comme `i64`, déréférencé par le premier consommateur `mixed` générique (`__mixed_to_float`, appelé par `var c:float = m["c"]`) : SIGSEGV.
- Le même défaut touchait AUSSI, à moindre gravité (pas de crash immédiat mais un risque similaire à celui déjà documenté pour `int`) : un `bool` (jamais boxé, bit brut `0`/`1` stocké tel quel — indiscernable de `null`/d'un petit entier authentique pour tout consommateur `mixed`) et un `int` assez grand pour être ambigu avec un pointeur boxé (même risque que `stdlib-sqlite-integer-column-boxing.md`, jamais déclenché par le repro initial car les valeurs `int` testées y étaient trop petites pour être dangereuses — confirmé dangereux séparément avec une valeur `int` réaliste, `1790255242`).
- Confirmé sur les DEUX formes de conteneur `mixed` (`map<string,mixed>` ET `array<mixed>`) — même branche de code partagée par les deux (`is_map_target` ne fait que choisir `__map_set` vs `__array_set`, le boxing manquant était en amont de ce choix).
- Confirmé NON-régressif sur un conteneur CONCRET (`array<float>`, `map<string,int>`) : `val` a déjà le type IR attendu par `__array_set`/`__map_set` dans ce cas, `box_for_any` y est un no-op (voir sa doc) — aucun changement de comportement.
- Type élément résolu via `elem_type_after_index` (`src/lower/expr.d/helpers.rs`, déjà utilisée pour la LECTURE indexée chaînée — voir `langage-index-chaine-sur-map.md`) plutôt qu'une nouvelle table : réutilise la résolution statique déjà existante (`Ident`/`Field` chaîné/`Index` chaîné) sans dupliquer de logique. Si le type élément ne peut pas être résolu statiquement (conteneur non couvert par cette fonction, ex. `getMap()[clé] = v`), `val` reste inchangé — comportement identique à avant ce correctif sur ce cas déjà hors périmètre.
- **6 tests unitaires Rust** ajoutés dans `src/lower/stmt.d/statements.d/assignments.rs` : float/bool/int assignés dans `map<string,mixed>` (vérifie l'émission de `__box_float`/`__box_bool`/`__box_int_for_mixed` avant `__map_set`), float assigné dans `array<mixed>` (même chemin, `__array_set`), non-régression `array<float>` concret (aucun boxing émis), non-régression type élément inconnu (aucun boxing émis, comportement historique).
- **12 assertions** dans un nouvel exemple `examples/tests/58_mixed_container_indexed_assignment_boxingTest.oc` : repro exact, tous les primitifs dans `map<string,mixed>` (string/int/float/bool×2/int dangereux), tous les primitifs dans `array<mixed>`, non-régression `array<float>` concret, non-régression `map<string,int>` concret.
- `cargo test -p ocara` : 134 passed (dont les 6 nouveaux).
- `make build` (les 4 crates) + `RUSTFLAGS="-D warnings"` : 0 warning.
- `./ci/regression.sh` : tous les tests noir-boîte passent. `./ci/unittests.sh examples/project/tests` : 50 PASS / 0 FAIL. `./ci/unittests.sh examples/tests` : 757 PASS / 0 FAIL, 0 ERREUR(S) — 745 PASS avant ce ticket, +12 PASS exactement les nouvelles assertions.
- Piège méthodologique rencontré PENDANT ce ticket, sans rapport avec le bug lui-même, documenté ici pour la prochaine fois : `ocaraunit` met en cache les binaires de test compilés dans `.ocaraunit_cache/`, un répertoire DIFFÉRENT selon le répertoire de travail depuis lequel il est lancé (`./.ocaraunit_cache/` à la racine, `examples/.ocaraunit_cache/` quand `ci/unittests.sh` s'exécute depuis `examples/`, etc.) — un `ocaraunit --clear` lancé depuis un mauvais répertoire ne vide donc PAS le cache réellement utilisé par `ci/unittests.sh`, et un nouveau binaire `ocara` reconstruit entre-temps peut sembler "ne pas prendre effet" alors que le compilateur est bien correct. Vérifié en confondant un instant ce symptôme de cache avec un troisième bug distinct ("le même code plante en méthode mais pas en fonction libre") — écarté après avoir vidé LES TROIS répertoires de cache (`./`, `examples/`, `examples/project/`) et reconstruit : comportement identique (correct) partout.

## Constat

Repro minimal (aucune classe, aucun SQLite) :

```ocara
var m:map<string, mixed> = {}
m["c"] = 3.14
var c:float = m["c"]   // SIGSEGV ici
```

```
Program received signal SIGSEGV, Segmentation fault.
0x0000000000408471 in __mixed_to_float ()
#0  0x0000000000408471 in __mixed_to_float ()
#1  0x0000000000407288 in main ()
```

## Cause

`src/lower/stmt.d/statements.d/assignments.rs`, `lower_assign`, avant correctif :

```rust
Expr::Index { object, index, .. } => {
    let obj_val = lower_expr(builder, object);
    let idx_val = lower_expr(builder, index);
    let is_map = is_map_target(builder, object);
    let func = if is_map { "__map_set" } else { "__array_set" };
    builder.emit(Inst::Call {
        dest:   None,
        func:   func.into(),
        args:   vec![obj_val, idx_val, val],   // ← `val` jamais boxé
        ret_ty: IrType::Void,
    });
}
```

À comparer avec la branche `Expr::Ident` juste au-dessus, dans la MÊME fonction, qui appelait déjà `box_for_any` depuis le début — la branche `Expr::Index` avait toujours été un angle mort de ce mécanisme.

## Correctif

Avant l'appel `__map_set`/`__array_set`, résoudre le type élément du conteneur via `elem_type_after_index(builder, object)` (déjà existante, utilisée pour la lecture indexée chaînée) puis appeler `box_for_any(builder, &IrType::from_ast(&elem_ty), val_ty, val)` — exactement la même fonction déjà utilisée par la branche `Expr::Ident`, désormais partagée par les deux formes d'affectation.

## Priorité / Complexité

**Terminé.** Priorité Haute (SIGSEGV sur un pattern de base du langage — construire une map/un array hétérogène par affectation indexée, l'un des usages les plus élémentaires de `mixed`). Complexité Légère : un seul appel manquant (`box_for_any`), déjà utilisé identiquement ailleurs dans le même fichier.

## Fichiers clés

`src/lower/stmt.d/statements.d/assignments.rs` (`lower_assign`, `Expr::Index`), `src/lower/expr.d/helpers.rs` (`elem_type_after_index`, jamais modifiée — déjà suffisante), `src/lower/stmt.d/statements.d/helpers.rs` (`box_for_any`, jamais modifiée — déjà correcte, simplement jamais appelée sur ce chemin), `examples/tests/58_mixed_container_indexed_assignment_boxingTest.oc`.
