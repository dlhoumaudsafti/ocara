# Remontée d'erreurs incohérente entre builtins

## Constat

Le langage documente un modèle homogène (toute exception a `message`/`code`/`source`, EBNF:1981) mais dans la pratique :

- **MySQL/MariaDB** : `MySQLException` est déclarée et documentée (`docs/builtins/MySQL.md`), mais **jamais levée** — toute erreur de requête (runtime/src/mysql.rs:74-243) est avalée silencieusement (`eprintln!` + retour `0`/tableau vide). `SQLite`, dans les mêmes situations, lève correctement `SQLiteException` — la comparaison directe des deux fichiers confirme que ce n'est pas une contrainte technique du driver mais un oubli.
- **YAML** : `YAML_decode` retourne `0` sur erreur de parsing (runtime/src/yaml.rs:124) sans jamais lever `YAMLException`, alors que `docs/builtins/YAML.md` documente un bloc `try/on e is YAMLException` qui ne se déclenchera jamais.
- **DotEnv** : `DotEnv_load` (runtime/src/dotenv.rs:47-54) fait un simple `eprintln!` sur fichier manquant ; `DotEnvException` est déclarée côté compilateur mais n'est même pas mentionnée dans sa propre documentation.

## Impact concret

Un développeur qui suit `docs/builtins/MySQL.md`/`YAML.md` à la lettre (bloc `try/on ExceptionType`) écrit du code de gestion d'erreur mort — les échecs de requête ne sont détectables qu'en lisant `stderr` ou en inspectant `affectedRows()`/des valeurs nulles a posteriori. L'exemple `examples/builtins/mysql.oc:101-109` illustre involontairement ce piège.

## Ampleur

Petit et mécanique : ajouter `throw_mysql_exception` (calqué sur `throw_sqlite_exception`, runtime/src/exception.rs:205-212) et remplacer les ~6 `eprintln!`/retours silencieux dans `mysql.rs` ; même chose pour YAML et DotEnv.

## Fichiers clés

`runtime/src/mysql.rs`, `runtime/src/yaml.rs`, `runtime/src/dotenv.rs`, `runtime/src/exception.rs`, `docs/builtins/{MySQL,YAML,DotEnv}.md`.
