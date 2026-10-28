# Builtin UI

Interface native décrite par un fichier `.oc.ui` (YAML, voir
[docs/oc-ui.md](../oc-ui.md)) : la même source s'affiche dans une fenêtre
Tauri sur desktop et en Jetpack Compose sur Android. Le code Ocara lie les
**actions** du fichier à des méthodes, lit et modifie l'état des nœuds.

```ocara
import ocara.UI
import ocara.UIEvent
```

## Exemple

`garage.oc.ui` (extrait) :

```yaml
screens:
  home:
    layout:
      type: column
      children:
        - { type: input, id: search, placeholder: "Rechercher", onSubmit: search }
        - type: list
          id: cars
          onSelect: open_car
          item: { type: text, bind: brand }
  details:
    layout:
      type: column
      children:
        - { type: text, id: car_title }
        - { type: button, text: "Retour", onClick: back }
```

<!-- doc-check: skip -->
```ocara
import ocara.UI
import ocara.UIEvent

class GarageController {
    public static method search(ui:UI, event:UIEvent): void {
        var query:string = ui.get("search", "value")
        ui.setItems("cars", CarService::find(query))
    }

    public static method openCar(ui:UI, event:UIEvent): void {
        var car:map<string, mixed> = event.value()
        ui.set("car_title", "value", car["brand"])
        ui.navigate("details")
    }
}

function main(): void {
    const ui:UI = use UI("garage.oc.ui")
    ui.on("search", GarageController::search)
    ui.on("open_car", GarageController::openCar)
    ui.run()
}
```

Exemple complet : [examples/ui/garage.oc](../../examples/ui/garage.oc).

## `use UI(path)`

Le fichier est lu **à la compilation** : une erreur du `.oc.ui` est une
erreur de compilation à sa ligne, et l'arbre normalisé est embarqué dans le
binaire (rien à livrer à côté, APK compris). `path` est un littéral ou une
constante ; un chemin relatif part du dossier du fichier `.oc`.

## Actions : `ui.on(action, handler)`

`handler` est une méthode statique `(ui:UI, event:UIEvent): void`, comme
un gestionnaire de route d'`HTTPServer`. L'action doit être déclarée dans le
`.oc.ui` (`onClick: search`…) ; sinon `UIException` 101. Plusieurs
gestionnaires d'une même action sont appelés dans l'ordre.

Actions particulières :

| Action | Sans gestionnaire | Avec `ui.on` |
|--------|-------------------|--------------|
| `back` | écran précédent ; sur l'écran de départ : rien (desktop), fermeture de l'app (Android) | le gestionnaire est appelé à la place sur l'écran de départ |
| `close` | ferme la fenêtre (l'app sur Android) | le gestionnaire décide (`ui.close()`) |
| `ready` | — | interface affichée (page chargée, premier rendu Compose) |

Les gestionnaires s'exécutent sur **un thread Ocara dédié, l'un après
l'autre** : un gestionnaire lent ne gèle pas l'interface, et deux
gestionnaires ne tournent jamais en même temps. Une exception non rattrapée
est signalée sur la sortie d'erreur ; les actions suivantes sont traitées.

## `UIEvent`

| Méthode | Résultat |
|---------|----------|
| `event.action()` → `string` | action déclarée |
| `event.id()` → `string` | `id` du nœud, `""` s'il n'en a pas |
| `event.value()` → `mixed` | texte saisi, `bool` d'une case, nombre d'un curseur, élément choisi d'une liste (`map`), `null` pour un clic |
| `event.index()` → `int` | rang de l'élément choisi (`onSelect`), -1 sinon |

## État des nœuds

| Méthode | Rôle |
|---------|------|
| `ui.get(id, prop)` → `mixed` | valeur courante : saisie de l'utilisateur, valeur posée par `set`, sinon celle du `.oc.ui`, sinon `null` |
| `ui.set(id, prop, value)` | change une propriété (`value`, `text`, `checked`, `visible`, `enabled`…) ; le nœud est réaffiché |
| `ui.setItems(id, rows)` | éléments d'une `list` (`array<map<string, mixed>>`, lus par `bind`) |

`id` doit exister dans le `.oc.ui` (sinon `UIException` 102). L'état est
gardé par le runtime : `get` ne questionne pas l'interface, et l'état posé
avant `run()` est affiché dès l'ouverture.

## Navigation et cycle de vie

| Méthode | Rôle |
|---------|------|
| `ui.navigate(screen)` | affiche l'écran (`UIException` 103 s'il n'est pas déclaré) |
| `ui.back()` | écran précédent |
| `ui.run()` | affiche l'interface et bloque jusqu'à sa fermeture |
| `ui.close()` | ferme l'interface : `run()` rend la main |

`set`, `setItems`, `navigate`, `back` et les boîtes de dialogue sont
permis depuis n'importe quel thread (gestionnaires compris).

## Services natifs

| Méthode | Desktop | Android |
|---------|---------|---------|
| `ui.toast(text)` | message bref en bas de la fenêtre | `Toast` |
| `ui.alert(title, text)` | boîte native, bloquante | `AlertDialog`, bloquante |
| `ui.confirm(title, text)` → `bool` | idem, vrai si validée | idem |
| `ui.pickFile(extensions)` → `string` | sélecteur natif, `""` si annulé | `UIException` 104 (pas encore) |
| `ui.saveFile(name)` → `string` | idem | `UIException` 104 (pas encore) |
| `ui.notify(title, text)` | notification système | notification (permission `notifications` requise, voir [AndroidNotifications](AndroidNotifications.md)) |

## Plateformes

- **Desktop** : l'objet `UI` est une fenêtre Tauri (options de
  `window:` et `window.tauri:`) dont la page est le moteur de rendu
  d'`ocara.UI` ; le programme est lié avec le runtime Tauri.
- **Android** : l'arbre est affiché par le moteur Compose de l'app de
  `packaging/android` à la place de sa WebView (options `window.android:`) ;
  le programme est lié avec `libocara_runtime_android_ui.a`
  (`make build-android-features`). Une seule interface est affichée à la
  fois.

Les spécificités de chaque plateforme restent dans [Tauri](Tauri.md) et
[AndroidUI](AndroidUI.md).

## Codes d'erreur UIException

| Code | Cause |
|------|-------|
| 101 | `ui.on` : action absente du `.oc.ui` |
| 102 | `get`/`set`/`setItems` : `id` absent du `.oc.ui` |
| 103 | `navigate` : écran absent du `.oc.ui` |
| 104 | service pas encore disponible sur cette plateforme |
| 105 | arbre d'interface invalide (binaire altéré) |

## Voir aussi

- [docs/oc-ui.md](../oc-ui.md) — format `.oc.ui`, aperçus, moteurs de rendu
- [Tauri](Tauri.md), [AndroidUI](AndroidUI.md)
