# `MySQL`/`MariaDB` — requêtes paramétrées + transactions prepare/bind/commit/rollback (bloqué par un prérequis architectural)

## Constat

Même manque que côté SQLite (voir [stdlib-sqlite-requetes-parametrees-transactions](stdlib-sqlite-requetes-parametrees-transactions.md) pour le design complet et l'exemple de référence — ce ticket applique le **même design** : placeholders nominatifs `:nom` uniquement, `execute`/`query`/`queryOne` avec `placeholder:map<string, mixed>|null = null`, plus `prepare`/`bind`/`commit(close:bool=false): mixed`/`rollback(close:bool=false)`).

**Ce ticket est délibérément séparé du ticket SQLite** parce que le risque et le prérequis ne sont pas les mêmes :

`runtime/src/mysql.rs` (`MySQL_execute`, `MySQL_query`, `MySQL_queryOne`) fait `db.pool.lock().unwrap().get_conn()` **à chaque appel** — chaque méthode récupère une connexion différente piochée dans le `Pool`. Un `prepare()`/`bind()`/`commit()` naïf plaqué sur ce modèle ne serait **pas réellement transactionnel** : rien ne garantit que le `BEGIN` (dans `prepare`/premier `bind`) et le `COMMIT` (dans `commit`) s'exécutent sur la même session serveur MySQL — deux connexions différentes du pool n'ont pas de `BEGIN` en commun. C'est un problème d'architecture, pas de détail d'implémentation.

## Ce qui est demandé

**Préalable obligatoire, à faire AVANT toute méthode `prepare`/`bind`/`commit`/`rollback`** : `OcaraMySQLDatabase` doit pouvoir épingler **une** connexion (`PooledConn`) pour la durée d'un cycle `prepare → bind → commit`/`rollback`, plutôt que de repiocher dans le pool à chaque appel. Deux pistes possibles, à trancher à l'implémentation :
- Un nouveau champ d'état (`Mutex<Option<PooledConn>>`) sur `OcaraMySQLDatabase`, posé par `prepare()`, consommé/relâché par `commit()`/`rollback()` — cohabite avec le `Pool` existant pour `execute`/`query`/`queryOne` one-shot qui n'ont pas besoin de cette garantie.
- Réévaluer si un `MySQL`/`MariaDB` Ocara doit continuer à représenter un `Pool` entier ou une connexion unique — chaque instance `MySQL` côté Ocara correspond déjà à *une* connexion logique du point de vue de l'utilisateur (un seul `MySQL::connect(...)`), le `Pool` interne est un détail d'implémentation actuel, pas une exigence du langage.

Une fois ce préalable posé, le reste du travail est le même que côté SQLite (7 méthodes, doc, exemples, tests) — voir [stdlib-sqlite-requetes-parametrees-transactions](stdlib-sqlite-requetes-parametrees-transactions.md) point par point, à adapter au crate `mysql` (paramètres nommés : `Params::from(...)` / requêtes préparées via `conn.prep(...)` + `conn.exec(...)`, transactions natives disponibles via `conn.start_transaction(...)` une fois la connexion épinglée).

## Priorité / Complexité

**Priorité Moyenne** — même justification sécurité que côté SQLite. **Complexité Structurel**, mais strictement postérieur au ticket SQLite : ne pas démarrer avant que le design prepare/bind/commit/rollback ait fait ses preuves côté SQLite (surface plus simple, une seule connexion déjà persistante) — le préalable de connexion épinglée ajoute un risque de régression sur `execute`/`query`/`queryOne` one-shot existants s'il est mal isolé (état de transaction fuité vers un appel one-shot ultérieur sur la même instance).

## Fichiers clés

`runtime/src/mysql.rs` (`OcaraMySQLDatabase`, préalable de connexion épinglée), `docs/builtins/MySQL.md`, `examples/builtins/mysql.oc`.
