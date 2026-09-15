# Remontée d'erreurs incohérente entre builtins

`MySQLException`/`MariaDBException` sont corrigées (levées réellement, layout mémoire correct) — voir git log pour le détail.

## Reste à faire : décision de design sur `YAMLException`/`DotEnvException`

`docs/builtins/YAML.md`/`DotEnv.md` documentent déjà, explicitement, que `YAML::decode`/`parse()` et `DotEnv::load()` ne lèvent **jamais** d'exception aujourd'hui (retour silencieux `null`/warning) — ce n'est pas un bug de cohérence comme MySQL l'était, mais deux classes d'exception déclarées côté compilateur et **orphelines** (jamais levées par aucun chemin runtime).

À trancher séparément (changement de contrat public, pas un simple correctif) :
- soit ces deux builtins doivent un jour réellement lever `YAMLException`/`DotEnvException` (et dans quels cas précisément) ;
- soit ces classes orphelines doivent être retirées du langage plutôt que de laisser croire qu'elles servent à quelque chose.

## Fichiers clés

`runtime/src/yaml.rs`, `runtime/src/dotenv.rs` (à vérifier), `docs/builtins/{YAML,DotEnv}.md`.
