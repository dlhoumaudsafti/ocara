# YAML : support partiel

## ✅ Corrigé

- `yaml_to_value` (decode) : un nombre YAML à virgule (`n.as_i64()` échoue) est maintenant reboxé via `__box_float` au lieu de silencieusement devenir `0`.
- `value_to_yaml` (encode) : une valeur `mixed` réellement boxée comme flottant (`__is_float`) est maintenant déboxée et encodée comme un nombre YAML, au lieu de tomber dans le cas par défaut.

Vérifié manuellement : `YAML::decode("pi: 3.14159")` restitue désormais `3.14159` (au lieu de `0`).

**Limite découverte en creusant, non corrigée ici** (hors périmètre "Légère", nouvelle fiche dédiée) : un littéral `float`/`bool` écrit directement dans un `array<mixed>`/`map<string, mixed>` (`{'pi': 3.14159}`) n'est **pas** boxé par le compilateur — il est stringifié au lowering, avant même d'atteindre YAML. `YAML::encode`/`JSON::encode` sur un tel littéral produisent donc `'3.14159'`/`"3.14159"` (chaîne quotée), pas un vrai nombre YAML/JSON. Documenté dans `docs/builtins/YAML.md` et détaillé dans [langage-mixed-literal-stringification](langage-mixed-literal-stringification.md).

## Types "tagged" — non traité, documenté

Les types YAML "tagged" (`!!something`) restent non supportés et retournent `Null` — comportement désormais documenté dans `docs/builtins/YAML.md` plutôt que traité (ampleur jugée disproportionnée pour l'usage réel du builtin).

## Fichiers clés

`runtime/src/yaml.rs`, `docs/builtins/YAML.md`.
