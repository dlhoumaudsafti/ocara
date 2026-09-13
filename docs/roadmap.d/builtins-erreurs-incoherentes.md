# Remontée d'erreurs incohérente entre builtins

## ✅ MySQL/MariaDB — corrigé

`MySQLException` (déclarée et documentée dans `docs/builtins/MySQL.md`, codes 101/102/103) n'était **jamais levée** — toute erreur de requête (`runtime/src/mysql.rs`) était avalée silencieusement (`eprintln!` + retour `0`/tableau vide), alors que `SQLite`, dans les mêmes situations, levait correctement `SQLiteException`.

**Corrigé** : `throw_mysql_exception` ajouté dans `runtime/src/exception.rs` (calqué sur `throw_sqlite_exception`) et branché sur les ~8 sites d'erreur de `runtime/src/mysql.rs` (`connect`/`execute`/`query`/`queryOne`, y compris les paramètres manquants et les erreurs de lecture de ligne). `MariaDB_*` en bénéficie automatiquement (délégation directe vers `MySQL_*`).

**Bug additionnel découvert et corrigé au passage** : `MySQLException` (et `MariaDBException`/`DotEnvException`/`YAMLException`) étaient absentes de la table `class_layouts` codée en dur dans `src/lower/builder.d/program.rs` (celle qui donne l'offset mémoire de `message`/`code`/`source`). Résultat : dès qu'une de ces exceptions était réellement levée et qu'on lisait `e.code` ou `e.source`, l'accès retombait sur l'offset 0 (`message`) faute de layout connu — `e.code` affichait le texte du message. Corrigé en ajoutant les 4 classes manquantes à cette table, avec la même disposition que les autres exceptions (`message`, `code`, `source`).

`examples/builtins/mysql.oc` mis à jour : `MySQL::connect(...)` doit maintenant être appelé à l'intérieur du `try` (comme le reste des opérations), puisqu'il peut désormais lever une exception au lieu de retourner `null`.

Vérifié : `make regression` ne montre aucune régression (comparé au build d'avant ces changements via `git stash`) ; test manuel confirmant qu'une connexion échouée est bien attrapée par `on e is MySQLException` avec `e.code == 101`.

## YAML et DotEnv — non modifiés (choix délibéré, pas un oubli)

En creusant plus loin, `docs/builtins/YAML.md` et `docs/builtins/DotEnv.md` documentaient en réalité **déjà** le comportement silencieux actuel comme intentionnel :
- `YAML::decode()`/`parse()` : la doc dit explicitement "retournent `null` (0)" en cas d'erreur de parsing — mais montrait *aussi*, en contradiction, un exemple `try/on e is YAMLException` qui ne se déclenche jamais.
- `DotEnv::load()` : la doc dit explicitement "un warning est affiché mais le programme continue" si le fichier `.env` est absent.

Changer ces deux comportements pour lever une exception serait un **changement de contrat public**, pas un simple correctif de cohérence (contrairement à MySQL, dont la doc n'a jamais mentionné de retour silencieux). Plutôt que de trancher ce choix de design unilatéralement, seule la documentation a été corrigée :
- `YAML.md` : suppression de l'exemple `try/on YAMLException` trompeur, clarification que `decode()`/`parse()` ne lèvent jamais d'exception aujourd'hui.
- `DotEnv.md` : précision que `load()` ne lève jamais `DotEnvException` (classe déclarée côté compilateur mais orpheline).

**Reste ouvert si on veut aller plus loin** : décider si `YAMLException`/`DotEnvException` doivent un jour être réellement levées (et dans quels cas), ou si ces classes orphelines doivent être retirées du langage. Non traité ici — décision de design à prendre séparément.

## Fichiers clés

`runtime/src/mysql.rs`, `runtime/src/exception.rs`, `src/lower/builder.d/program.rs`, `docs/builtins/{MySQL,YAML,DotEnv}.md`, `examples/builtins/mysql.oc`.
