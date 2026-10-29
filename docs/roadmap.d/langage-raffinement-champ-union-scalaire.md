# Raffinement `is int` sur un champ `int|null` : le générateur de code appelle `String_toStr`

## Constat

La sema accepte le raffinement d'un **champ** typé `int|null`, mais le
générateur de code résout la méthode sur le mauvais type et échoue :

```ocara
architecture permissive
import ocara.IO

class Dto {
    public property id:int|null
    init(id:int|null) { self.id = id }
}

class Main {
    public static method main():int {
        const d:Dto = use Dto(7)
        if d.id is int {
            IO::writeln(`id ${d.id.toStr()}`)
        }
        return 0
    }
}
```

```text
main.oc:0:0: error: codegen: codegen error: appel vers une fonction inconnue : String_toStr
```

Le même raffinement sur une **variable locale** fonctionne
(`const id:int|null = d.id; if id is int { id.toStr() }`) : c'est le
contournement utilisé par `examples/advanced/mini_project_ui_hexa`
(`context/car/infra/view/CarView.oc`).

## Pistes

- Le raffinement par `is` n'est enregistré que pour un `Expr::Ident` ; un
  `Expr::Field` garde son type union au lowering, et l'appel de méthode sur
  l'union retombe sur `String_*`.
- L'erreur sans position (`0:0`) est elle-même à corriger.

## À faire

- Raffiner les accès de champ (`a.b`, `self.b`) comme les variables, ou
  refuser le raffinement d'un champ à la sema avec un message clair.
- Test de non-régression dans `examples/tests/`.
