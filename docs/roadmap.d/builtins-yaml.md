# YAML : support partiel

- `value_to_yaml`/`yaml_to_value` ne traitent que `Number::as_i64()` (runtime/src/yaml.rs:143-146) : un flottant YAML retombe silencieusement à `0`.
- Les types YAML "tagged" (`!!something`) ne sont pas supportés et retournent `Null` (runtime/src/yaml.rs:171).
- Voir aussi [builtins-erreurs-incoherentes](builtins-erreurs-incoherentes.md) pour l'absence de `YAMLException` levée.

## Ampleur

Léger : gérer `Number` flottant dans la conversion, documenter ou traiter le cas des types tagués.
