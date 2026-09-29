# `db.query()`/`db.queryOne()` : une colonne INTEGER assez grande fait planter le processus (SIGSEGV)

Vérifié :
- Reproduit à l'identique le repro fourni (`gdb -q -batch -ex run -ex bt`) : `SIGSEGV` dans `__mixed_to_int`, en lisant `row["created_at"]` (colonne `INTEGER` valant `1790255242`) dans un `var createdAt:int`, après un `db.query("SELECT * FROM maintenances")` à une seule ligne.
- Root cause confirmée (PAS celle d'abord suspectée — "6 conversions et au moins un float" était une fausse piste, voir plus bas) : `collect_all_rows` (`runtime/src/sqlite.rs`, chemin de `query()`/`commit()` SELECT) ET `SQLite_queryOne` (copie séparée de la même logique) stockaient la valeur `i64` brute d'une colonne `INTEGER` (`row.get::<_, i64>(i)`) directement dans le `mixed` de la map résultat, SANS jamais la faire passer par `box_int_if_needed` (`runtime/src/lib.rs`) — contrairement à la colonne `REAL`, déjà correctement boxée (`__box_float`) depuis un correctif antérieur (voir `stdlib-sqlite-real-column-boxing`/`examples/tests/55_sqlite_real_column_boxingTest.oc`).
- Conséquence exacte : un entier `>= 0x10000` (`PTR_THRESHOLD`) dont les 2 bits bas valent `01`/`10`/`11` devient indiscernable d'un float/bool/int **boxé** (voir `is_float_box`/`is_bool_box`/`is_int_box`). `1790255242 & 3 == 2` → confondu avec un `bool` boxé par `is_bool_box`. Tout consommateur `mixed` générique (`__mixed_to_int`/`unbox_numeric_i64`, appelé par le lowering dès qu'une valeur `mixed` est affectée à un `int` concret — exactement `var createdAt:int = row["created_at"]`) le déballe alors comme si c'était un pointeur boxé et le **déréférence** à l'adresse `1790255242 & !3 = 1790255240` — une adresse arbitraire, non mappée : SIGSEGV.
- **La piste initiale ("6 conversions, au moins un float présent") était fausse et abandonnée à la demande** — vérifié explicitement : un repro à 2 lignes / 4 colonnes SANS AUCUN float (int/text/text/int) plante aussi dès qu'une des colonnes entières dépasse `PTR_THRESHOLD` avec les mauvais bits bas ; un repro à 5 lignes couvrant délibérément les 4 classes de bits bas (`& 3` = 0/1/2/3) plus un timestamp réaliste, avec ou sans colonne `REAL`, confirme que la seule variable causale est la **valeur** d'une colonne entière quelque part dans les données — jamais le nombre de colonnes lues, le nombre de lignes, ni la présence d'un float. Le motif "6 conversions + float" observé au départ était un artefact des valeurs de test précises utilisées (le seul entier dangereux, `created_at`, se trouvait être la 6ᵉ colonne lue) ; le motif "row 2 crashe" observé ensuite sur un tri `ORDER BY ... DESC` était un artefact de l'ORDRE d'émission des lignes (la ligne dont le timestamp a les bons bits bas se trouvait être la seconde après le tri), pas une corruption entre lignes.
- **Hypothèse "connexion réutilisée corrompt un résultat déjà référencé" écartée par reproduction** — vérifiée avec des valeurs `int`/`string` PETITES (jamais ambiguës) sur : deux `queryOne()` séquentiels sur la même connexion en lisant le premier résultat APRÈS le second ; un cycle open/query/close répété 3 fois en gardant une référence à chaque résultat après la fermeture de sa connexion d'origine. Aucun crash, aucune corruption dans les deux cas — confirme que les chaînes (`alloc_str`, une vraie copie heap indépendante des buffers `sqlite3_column_*`, jamais un pointeur dedans) et les entiers sûrs survivent correctement à la fermeture de connexion/à une requête suivante. Un nouveau test avec des IDs/timestamps VOLONTAIREMENT dangereux dans ce même scénario (`repro_reuse_large_ids`) confirme que le symptôme "connexion réutilisée" rapporté était très probablement la MÊME cause (des données réelles avec de grands entiers), pas un second bug d'aliasing/UAF séparé — aucune fuite de buffer/pointeur SQLite trouvée dans `alloc_str`/`box_int_if_needed`/`__box_float` (chacun alloue sa PROPRE cellule heap indépendante, aucun état partagé/statique).
- Hypothèse "colonne calculée `CAST(... AS TEXT)`" testée séparément : aucun crash observé, colonne correctement lue comme texte. Pas de second bug distinct trouvé sur ce point.
- Corrigé par une fonction de conversion partagée, `row_column_as_mixed` (`runtime/src/sqlite.rs`), appelant `crate::box_int_if_needed` sur la branche entière — utilisée par `collect_all_rows` ET `SQLite_queryOne`, qui dupliquaient chacune leur propre copie de cette logique (déjà la cause du bug analogue, corrigé une première fois pour `REAL` mais jamais répercuté à `INTEGER` — le commentaire historique "Voir le commentaire équivalent dans collect_all_rows" documentait déjà cette duplication sans l'éliminer).
- **6 tests unitaires Rust** ajoutés dans `runtime/src/tests/sqlite.rs` (Groupe 3) : le repro exact (une ligne, colonnes mixtes dont un `INTEGER` dangereux et un `REAL`), la même chose via `queryOne`, **une requête à 5 lignes couvrant les 4 classes `& 3`** (le point explicitement signalé comme jamais testé nulle part dans ce projet), un entier négatif (jamais ambigu, non-régression), un entier `0` authentique (non confondu avec `null`, non-régression). Avant le correctif, ces tests faisaient planter le PROCESSUS de test lui-même (`SIGSEGV`, vérifié en rejouant `cargo test` sur l'ancien code via `git stash`) — le signal de régression le plus fort possible pour ce genre de bug, pas une simple assertion qui échoue.
- **31 assertions** dans un nouvel exemple `examples/tests/57_sqlite_integer_column_boxingTest.oc` : repro exact (`query`), repro exact (`queryOne`), requête multi-lignes (5 lignes, 4 classes de bits bas + un timestamp réaliste, avec colonne `REAL`), table réaliste `cars` à 3 lignes et 2 colonnes `REAL` (forme exacte signalée par l'application qui a révélé le bug), entier négatif, entier `0`.
- `cargo test -p ocara_runtime` : 63 passed (dont les 6 nouveaux), 7 `#[ignore]` (inchangé, MySQL). `cargo test -p ocara` : 128 passed (inchangé, aucun fichier de `src/` touché par ce ticket).
- `make build` (les 4 crates) + `RUSTFLAGS="-D warnings"` : 0 warning.
- `./ci/regression.sh` : tous les tests noir-boîte passent. `./ci/unittests.sh examples/project/tests` : 50 PASS / 0 FAIL. `./ci/unittests.sh examples/tests` : 745 PASS / 0 FAIL, 0 ERREUR(S) — 714 PASS avant ce ticket, +31 PASS exactement les nouvelles assertions.

## Constat

Repro minimal (une seule ligne, aucune classe) :

```ocara
db.execute("CREATE TABLE maintenances (id INTEGER PRIMARY KEY, car_id INTEGER, type TEXT, description TEXT, cost REAL, created_at INTEGER)")
db.execute("INSERT INTO maintenances (car_id, type, description, cost, created_at) VALUES (1, 'improvement', 'Peinture', 500.0, 1790255242)")

const rows:array<map<string, mixed>> = db.query("SELECT * FROM maintenances")
for row in rows {
    var createdAt:int = row["created_at"]   // SIGSEGV ici
}
```

```
Program received signal SIGSEGV, Segmentation fault.
0x0000000000408de4 in __mixed_to_int ()
#0  0x0000000000408de4 in __mixed_to_int ()
#1  0x000000000040815f in main ()
```

## Cause

`runtime/src/sqlite.rs`, `collect_all_rows` (avant correctif) :

```rust
let value = if let Ok(v) = row.get::<_, i64>(i) {
    v   // ← stocké BRUT, jamais boxé
} else if let Ok(v) = row.get::<_, f64>(i) {
    crate::__box_float(v.to_bits() as i64)   // déjà correct
} else if let Ok(v) = row.get::<_, String>(i) {
    unsafe { alloc_str(&v) }
} else {
    0
};
```

`SQLite_queryOne` avait une copie IDENTIQUE de ce même bloc (avec le même bug). L'invariant `mixed` (déjà documenté au-dessus, sur `mixed_to_sql_value`) exige qu'un entier logé dans un `mixed` soit boxé dès qu'il vaut `0` ou dépasse `PTR_THRESHOLD` (`0x10000`) — sinon ses 2 bits bas peuvent être confondus avec le tag d'un float/bool/int boxé par n'importe quel code qui traite la valeur comme `mixed` générique. `__mixed_to_int`/`unbox_numeric_i64` (`runtime/src/lib.rs`) fait exactement ça dès qu'un résultat de requête est affecté à un `int` concret — déréférençant alors une adresse arbitraire.

## Correctif

`row_column_as_mixed(row, i)` (nouvelle fonction, `runtime/src/sqlite.rs`) applique `crate::box_int_if_needed(v)` sur la branche entière, et est appelée par `collect_all_rows` ET `SQLite_queryOne` — un seul point pour les deux chemins, au lieu de deux copies divergentes.

## Priorité / Complexité

**Terminé.** Priorité Haute (SIGSEGV, pas seulement une donnée fausse — mémoire non sûre sur le chemin `SQLite::query()`/`queryOne()` officiellement documenté). Complexité Légère : un seul appel manquant (`box_int_if_needed`), déjà utilisé partout ailleurs dans le runtime pour exactement cet invariant — aucune réécriture d'architecture.

## Fichiers clés

`runtime/src/sqlite.rs` (`row_column_as_mixed`, `collect_all_rows`, `SQLite_queryOne`), `runtime/src/lib.rs` (`box_int_if_needed`, `unbox_numeric_i64`, `__mixed_to_int`, jamais modifiés — le bug était uniquement dans l'appelant qui omettait de les utiliser), `runtime/src/tests/sqlite.rs` (Groupe 3), `examples/tests/57_sqlite_integer_column_boxingTest.oc`.
