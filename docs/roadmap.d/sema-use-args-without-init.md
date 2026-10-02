# Arguments de `use Classe(...)` ignorés pour une classe sans `init`

## Constat

Trouvé en testant `ocaracs --fix` :

```ocara
class CarModel {
    public property name:string
}

function main(): int {
    var c:CarModel = use CarModel("x")   // compile sans erreur
    IO::writeln(c.name)                  // affiche "null"
    return 0
}
```

La classe n'a pas de constructeur. `use CarModel("x")` est pourtant accepté,
et l'argument est silencieusement perdu. Pour un `struct`, le constructeur
généré à partir des champs est bien vérifié (arité). Pour une classe avec
`init`, l'arité aussi.

## À trancher

- **Erreur d'arité** (`'CarModel' has no init() — use CarModel() without
  arguments`), le plus simple et cohérent avec une classe sans constructeur.
- Ou **constructeur par champs implicite**, comme un `struct`. Cela
  rapprocherait encore `class` et `struct` (voir
  [langage-struct-value-type](langage-struct-value-type.md)), mais l'ordre des
  `property` deviendrait une API publique.

## Priorité / Complexité

Haute (erreur silencieuse) — **Simple** pour l'erreur d'arité.

## Fichiers clés

`src/sema/typecheck.rs` (`Expr::New`), `src/core/structs.rs` (vérification
d'arité déjà faite pour les `struct`), `src/core/property_init.rs` (`init`
synthétisé pour les initialiseurs de `property`).
