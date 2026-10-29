# Raffinement `is int` sur un champ `int|null` : le générateur de code appelle `String_toStr`

Statut : **corrigé** le 2026-10-29 (constaté le 2026-10-29).

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

## Correctif

- **Sema** : le raffinement porte aussi sur un chemin de champ partant d'une
  variable ou de `self` (`d.id`, `self.owner.id`) — `sema::narrowing::path_of`,
  pile des portées (`sema/scope.rs`) indexée par chemin, type de départ pris
  au dernier type inféré du chemin (`TypeChecker::path_types`). Une
  affectation de `d` ou de `d.id` annule le raffinement de `d.id`.
- **Sema** : un appel de méthode sur une union de scalaires non raffinée
  (`int|null`) est refusé (E37) au lieu de retomber en silence sur `mixed`.
- **Lowering** : un argument `mixed`/union passé à un paramètre `int`/`float`/
  `bool` est déboxé (`lower_call_arg_at`). Défaut voisin trouvé en route :
  `r.toStr()` sur un `float|null` raffiné (même une variable locale)
  affichait le pointeur de la boîte, et un grand `int` boxé aussi.

Tests : `examples/tests/122_field_narrowingTest.oc` (8 assertions),
`src/sema/tests/method_call_on_non_class.rs` (3 tests). Contournement
retiré de `examples/advanced/mini_project_ui_hexa` (`CarView.oc`).
