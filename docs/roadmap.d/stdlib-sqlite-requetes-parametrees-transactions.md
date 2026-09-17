# `SQLite` — requêtes paramétrées (placeholders nominatifs) + transactions prepare/bind/commit/rollback

## ✅ Terminé (points 1, 2, 3, 5 — point 4 explicitement différé, voir plus bas)

Les 7 méthodes implémentées exactement selon les signatures décidées ci-dessous (`runtime/src/sqlite.rs`, `src/builtins/sqlite.rs`, `src/codegen/desc.d/sqlite.rs`). Découverte en cours de route non anticipée par le design initial : le mécanisme de dispatch par arité pour les builtins à paramètres optionnels (suffixe `_N`, déjà utilisé par `DotEnv::load`) n'existait que pour les appels de fonction libre et `Class::method()` statique — jamais pour `obj.méthode()`. Généralisé dans `src/lower/expr.d/lower.rs` (juste avant l'émission de `Inst::Call` dans la branche d'appel de méthode) : recherche directe d'une variante `<nom>_<N réel>` plutôt que de réutiliser le calcul `args.len() < builtin.params.len()` des deux autres branches, qui aurait été décalé d'un cran par `self` (absent de `args` mais présent dans `builtin.params`, propre aux méthodes d'instance).

Vérifié :
- `make build` (les 4 crates) + `RUSTFLAGS="-D warnings"` : 0 warning.
- `make regression` (cache vidé au préalable) : 659 PASS / 0 FAIL / 0 ERREUR côté `ci/unittests.sh`, 64 OK / 0 KO côté `ci/regression.sh` — aucune régression sur le reste du corpus malgré la modification partagée de `lower.rs`.
- Test manuel bout en bout (compilé + exécuté, pas seulement `--check`) : `execute`/`query`/`queryOne` avec et sans `placeholder`, flux `prepare`/`bind`/`commit` réussi sur INSERT et sur SELECT (retour `mixed` auto-détecté vérifié dans les deux cas), échec réel (violation `UNIQUE`) intercepté par `on e is SQLiteException` puis `rollback()` — connexion vérifiée réutilisable ensuite, ligne en échec bien absente après rollback. `prepare()` sur SQL invalide et `bind()` sans `prepare()` préalable lèvent chacun l'exception attendue (105/106) avec un message exploitable.
- L'exemple de référence de `docs/builtins/SQLite.md` (virement entre deux comptes) compilé et exécuté séparément — résultat numérique vérifié.
- `examples/builtins/sqlite.oc` étendu (sections 10/11) et exécuté en entier via son cycle `init`/`main`/`success`/`exit`.
- `python3 tools/highlight/vsode/scripts/generate-builtins-data.py` relancé (`SQLite : 12 méthodes (2 statiques, 10 instance)`).
- **Tests unitaires Rust dédiés** (`runtime/src/tests/sqlite.rs`, 19 tests, `cargo test -p ocara_runtime` : 44 passed — les 25 tests `boxing` existants + les 19 nouveaux, 0 warning) : `mixed_to_sql_value`/`read_placeholders` testées en isolation sur les cas frontière du bit pattern `mixed` propres au bind SQL (`0` = `null` vs entier `0` boxé, brut vs boxé, `array`/`map` rejetés) ; les fonctions `SQLite_*` réelles exercées sur base `:memory:` pour les chemins de succès (round-trip `execute`/`query`, binding nommé, `prepare`/`bind`/`commit` sur INSERT et SELECT, `rollback()` à vide, deuxième cycle `prepare` après un premier `commit`). Volontairement **pas** de test direct des chemins d'erreur des fonctions FFI (`throw_sqlite_exception` saute par `longjmp` vers un `jmp_buf` établi par le code généré pour un `try` Ocara — absent d'un `#[test]` Rust ordinaire, comportement indéfini garanti) : ces chemins restent couverts par les tests `.oc` compilés+exécutés ci-dessus, qui ont le vrai contexte `setjmp` en place.

**Point 4 explicitement différé** : `examples/advanced/mini_project/helpers/SqlSafe.oc` et ses appelants (`models/Car.oc`, `models/Maintenance.oc`) n'ont **pas** été migrés vers le binding natif. Raison : périmètre déjà large pour un seul ticket (implémentation runtime + lowering + doc + exemple builtin) ; migrer un projet d'exemple complet est un changement séparé, à faible risque de régression mais qui mérite sa propre vérification (relire tout `mini_project`, pas juste `SqlSafe.oc`). Si repris : supprimer `SqlSafe::escape()` au profit de `placeholder:map<...>` partout où `models/Car.oc` concatène aujourd'hui, et vérifier `make regression advanced/mini_project` (ou équivalent) après coup.

## Constat

`db.execute(query: string)`, `db.query(query: string)` et `db.queryOne(query: string)` (`docs/builtins/SQLite.md:40,53,69`) n'acceptent qu'une chaîne SQL déjà entièrement construite — aucun binding de paramètre. Toute valeur variable doit être concaténée et échappée à la main (voir `examples/advanced/mini_project/helpers/SqlSafe.oc`, qui n'existe que pour ça). Rien non plus pour grouper plusieurs opérations en une transaction sûre (commit/rollback) — une exception au milieu d'une séquence de plusieurs `execute()` laisse la base dans un état partiel.

Design décidé en conversation (voir aussi [stdlib-mysql-requetes-parametrees-transactions](stdlib-mysql-requetes-parametrees-transactions.md) pour le pendant MySQL — **ticket distinct**, prérequis architectural différent, voir sa fiche) :

- **Uniquement des placeholders nominatifs `:nom`** — pas de `?` positionnel (jugé moins lisible, en contradiction avec le parti pris du langage sur les opérateurs en toutes lettres plutôt que des symboles).
- **Deux façons d'utiliser le binding**, sur les mêmes objets `SQLite` déjà existants (pas de nouvelle classe/ressource) :
  - **One-shot** : `execute`/`query`/`queryOne` acceptent un paramètre optionnel `placeholder:map<string, mixed>|null = null` — un seul appel, rétrocompatible (les appels existants sans 2ᵉ argument continuent de marcher tels quels).
  - **Stepped/transactionnel** : nouvelles méthodes `prepare(query:string)`, `bind(placeholders:map<string, mixed>)`, `commit(close:bool = false): mixed`, `rollback(close:bool = false)`.

### Signatures finales

```
public method execute(query:string, placeholder:map<string, mixed>|null = null, close:bool = false): void
public method query(query:string, placeholder:map<string, mixed>|null = null): array<map<string, mixed>>
public method queryOne(query:string, placeholder:map<string, mixed>|null = null): map<string, mixed>

public method prepare(query:string): void
public method bind(placeholders:map<string, mixed>): void
public method commit(close:bool = false): mixed
public method rollback(close:bool = false): void
```

### Usage attendu (exemple de référence de la conversation)

```ocara
const db:SQLite = SQLite::open(self::path())
try {
    db.prepare("SELECT * FROM table WHERE id = :id")   // erreur de syntaxe → SQLiteException immédiat
    db.bind({"id": 156})                                 // placeholder inconnu/type incompatible → SQLiteException
    var result:mixed = db.commit()                       // exécute + commit — voir ci-dessous pour le type de retour
} on e is SQLiteException {
    IO::writeln(`Error : ${e}`)
    db.rollback()   // annule la transaction en cours ; close:false par défaut (ne ferme PAS la connexion sauf rollback(true))
}
```

### `commit()` retourne `mixed`, auto-détecté

Décidé explicitement (pas de `commitQuery`/`commitQueryOne` séparées) : `commit()` retourne `array<map<string, mixed>>` si la requête préparée produit des colonnes (SELECT — détectable via `Statement::column_count() > 0` côté `rusqlite`), sinon un `int` (lignes affectées), exactement comme `execute()` aujourd'hui côté MySQL. Le code appelant Ocara doit déclarer la variable réceptrice en `mixed` (ou `is`-narrower ensuite).

### Comportement de `close`

`close:bool = false` sur `execute`/`commit`/`rollback` : ferme la connexion **seulement si demandé explicitement** — ce n'est pas automatique par défaut. (Le commentaire d'exemple de la conversation, *"on annule la requête et on close()"* sur `rollback()`, illustrait le cas `close:true` — la signature `rollback(close:bool = false)` fait foi : pas de fermeture implicite.)

## Notes d'implémentation (`runtime/src/sqlite.rs`)

- `OcaraSQLiteDatabase` gagne un état mutable pour le flux stepped : la requête en attente (texte, posé par `prepare()`) et les binds nommés (map posée par `bind()`), sous le même `Mutex` que la connexion — pas de `rusqlite::Statement` gardée vivante entre les appels FFI (ça emprunterait la connexion sur une durée de vie qui ne survit pas à un appel C séparé). `prepare()` compile quand même la requête une première fois via `conn.prepare(&query)` immédiatement pour faire remonter une erreur de syntaxe tout de suite (comme demandé), puis relâche ce `Statement` — `commit()` re-prépare avec les binds au moment de l'exécution (`rusqlite` prépare vite, ce n'est pas un problème de perf à ce niveau).
- `bind()` convertit `map<string, mixed>` en paramètres nommés `rusqlite` (`&[(&str, &dyn ToSql)]` / `named_params!`) — conversion dynamique par tag de type (`mixed` est déjà tagué pointeur/int/float/bool/string, voir `runtime/src/typecheck.rs`).
- `commit()` encadre l'exécution d'un vrai `BEGIN`/`COMMIT` SQL (pas juste une exécution simple) pour que `rollback()` ait quelque chose de réel à annuler en cas d'échec **avant** le `COMMIT` final. Sur échec, `commit()` lève l'exception SANS rollback automatique — c'est `rollback()`, appelé explicitement depuis le `on`, qui nettoie (cohérent avec l'exemple).
- Nouveaux codes d'erreur `SQLiteException` documentés dans `docs/builtins/SQLite.md` : `PREPARE` (105), `BIND` (106), `COMMIT` (107) — à la suite de `OPEN`/`EXECUTE`/`QUERY`/`CLOSE` (101-104) existants. Pas de code dédié pour `rollback()` : un échec du `ROLLBACK` lui-même est absorbé silencieusement (best-effort) plutôt que de masquer l'exception d'origine qui a mené à l'appeler. `docs/diagnostics.md` n'a pas eu besoin de modification — il ne fait que pointer vers `SQLite.md`, déjà à jour.

## Ce qui est demandé

1. Implémenter les 7 méthodes ci-dessus dans `runtime/src/sqlite.rs` + déclarations `src/builtins/`.
2. Mettre à jour `docs/builtins/SQLite.md` (nouvelles méthodes, nouveaux codes d'erreur, exemple prepare/bind/commit/rollback).
3. Mettre à jour `examples/builtins/sqlite.oc` avec une démonstration des deux flux (one-shot et stepped).
4. Faire évoluer `examples/advanced/mini_project/helpers/SqlSafe.oc` et ses appelants (`models/Car.oc`, `models/Maintenance.oc`) pour utiliser le binding natif au lieu de l'échappement manuel — ou documenter explicitement pourquoi on les laisse tels quels si ce n'est pas fait dans ce ticket.
5. Tests de régression : au moins un exemple `.oc` couvrant le succès et l'échec (bind sur placeholder inconnu, erreur SQL pendant `commit()` suivie de `rollback()`).

## Priorité / Complexité

**Terminé** (points 1/2/3/5 ; point 4 différé, voir plus haut). Était Priorité Moyenne, Complexité Structurel — confirmé, plus une extension non anticipée du mécanisme de dispatch par arité (`src/lower/expr.d/lower.rs`), nécessaire pour que `execute`/`query`/`queryOne`/`commit`/`rollback` à paramètres optionnels fonctionnent en tant que méthodes d'INSTANCE (jusque-là seulement supporté pour fonctions libres/méthodes statiques).

## Fichiers clés

`runtime/src/lib.rs` (visibilité élargie de `is_float_box`/`is_bool_box`/`is_int_box`/`unbox_*`/`get_value_type`/`box_int_if_needed` en `pub(crate)`, + `map_entries`), `runtime/src/sqlite.rs`, `runtime/src/tests/sqlite.rs` (nouveau, 19 tests) + `runtime/src/tests/mod.rs` (`mod sqlite;`), `src/builtins/sqlite.rs`, `src/codegen/desc.d/sqlite.rs`, `src/lower/expr.d/lower.rs` (dispatch par arité généralisé aux méthodes d'instance), `docs/builtins/SQLite.md`, `examples/builtins/sqlite.oc`, `tools/highlight/vsode/data/builtins-data.json` (régénéré). Non touché (différé, voir plus haut) : `examples/advanced/mini_project/helpers/SqlSafe.oc` et ses appelants.
