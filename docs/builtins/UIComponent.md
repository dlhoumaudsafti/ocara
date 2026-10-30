# `ocara.UIComponent` — Classe builtin

> Comportement d'un composant déclaré sous `components:` d'un `.oc.ui`
> ([docs/oc-ui.md](../oc-ui.md), « Composants ») : ses **actions propres**
> et ses **instances**. Obtenu par `ui.component("nom")` ; valeur native
> comptée, liée à son [`UI`](UI.md).

---

## Import

```ocara
import ocara.UIComponent
```

---

## Actions propres

Dans le `layout` d'un composant, une action écrite `self.nom` appartient au
composant : le compilateur la nomme `<composant>.nom`. Toutes les instances
la partagent ; `event.instance()` dit laquelle l'a déclenchée.

```yaml
components:
  carCard:
    params: { title: text }
    layout:
      type: card
      children:
        - { type: text, id: title, value: $title }
        - { type: iconButton, icon: delete, label: "Supprimer", onClick: self.remove }

screens:
  home:
    layout:
      type: column
      children:
        - { type: carCard, id: clio, title: "Clio" }
        - { type: carCard, id: polo, title: "Polo" }
```

```ocara
const cards:UIComponent = ui.component("carCard")
cards.on("remove", CarCardController::remove)

class CarCardController {
    public static method remove(ui:UI, event:UIEvent): void {
        // "clio" ou "polo" ; le titre : ui.get("clio.title", "value")
        ui.set(event.instance(), "visible", false)
    }
}
```

Un nœud qui porte une action propre sans `id` en reçoit un quand
l'instance en a un (`clio.remove`) : `event.instance()` la retrouve. Une
instance sans `id` : `event.instance()` vaut `""`.

---

## Méthodes

### `ui.component(name: string)` → `UIComponent`

Composant déclaré dans le `.oc.ui` (ou un fichier inclus). Avec un
littéral, le nom est vérifié à la compilation ; sinon `UIException` 106
s'il est inconnu.

### `c.on(action: string, handler)` → `void`

Lie l'action propre `self.action` du composant ; même gestionnaire que
[`ui.on`](UI.md) : `(ui:UI, event:UIEvent): void`, exécuté sur le thread
des actions. `UIException` 101 si le composant n'a pas cette action.

### `c.name()` → `string`

Nom du composant.

### `c.instances()` → `array<string>`

`id` des instances posées avec un `id`, toutes variantes de taille
réunies.

### `c.id(instance: string, inner: string)` → `string`

`id` d'un nœud interne d'une instance : `instance.inner` (`inner` vide :
l'instance elle-même), à passer à `ui.get`/`ui.set`.

```ocara
for card in cards.instances() {
    ui.set(cards.id(card, "title"), "bold", true)
}
```

---

## Voir aussi

- [UI](UI.md) — `ui.on`, `ui.get`/`ui.set`, `UIEvent`
- [docs/oc-ui.md](../oc-ui.md) — déclarer un composant (`components:`)
