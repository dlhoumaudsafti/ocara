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

## `use UI(path, vars)`

`vars` (`map<string, mixed>`, facultatif) remplit les variables `${nom}`
des textes du `.oc.ui` ; `ui.setVars(vars)` les change ensuite (ajoutées aux
précédentes), l'interface est réaffichée, état et écran gardés :

<!-- doc-check: skip -->
```ocara
const ui:UI = use UI("garage.oc.ui", {"title": "Mes voitures"})
ui.setVars({"title": "My cars"})
```

Le fichier peut inclure des écrans d'autres fichiers et avoir des variantes
pour les grands écrans (`garage.md.oc.ui`…) : voir
[docs/oc-ui.md](../oc-ui.md), « Écrans inclus », « Variables » et
« Tailles d'écran ». `ui.sizeClass()` → `string` donne la classe de taille
courante (`xxs` … `xxl`, `""` avant l'affichage) ; l'action prédéfinie
`resize` prévient d'un changement.

`ui.has(id)` évite de calculer une donnée que l'interface affichée ne
montre pas : une variante peut avoir un nœud de plus (une photo dans les
cartes d'une liste, par exemple). Le relancer à l'action `resize`, la
variante pouvant changer :

```ocara
ui.on("resize", nameless(u:UI, event:UIEvent): void {
    // `car_photo` : image du gabarit de la liste, dans garage.lg.oc.ui seulement.
    u.setItems("cars", Cars::rows(u.has("car_photo")))
})
```

Le fichier est lu **à la compilation** : une erreur du `.oc.ui` est une
erreur de compilation à sa ligne, et l'arbre normalisé est embarqué dans le
binaire (rien à livrer à côté, APK compris ; les images locales aussi, voir
[docs/oc-ui.md](../oc-ui.md)). `path` est un littéral ou une constante ; un
chemin relatif part du dossier du fichier `.oc`.

Sur la variable initialisée par `use UI(…)`, les appels avec un littéral sont
vérifiés dès la compilation : action de `ui.on`, `id` de `get`/`set` (d'une
`list` pour `setItems`, d'une `webview` pour `postMessage`/`eval`…), écran
de `navigate`, racine de `bind`/`unbind` (et ses chemins liés, voir
[Liaisons](#liaisons)). Ailleurs (gestionnaire qui reçoit `ui:UI`, nom calculé), la
vérification se fait à l'exécution (`UIException` 101 à 103).

**Rechargement à chaud** : un programme compilé sans `--release` surveille
son `.oc.ui` pendant `ui.run()` (desktop) ; chaque modification est
revalidée par le compilateur et l'interface est réaffichée, état,
gestionnaires et écran courant gardés. Une erreur s'affiche sur la sortie
d'erreur et l'interface reste telle quelle. Le binaire de debug contient
alors le chemin du `.oc.ui` et du compilateur ; un build `--release` non.

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
| `resize` | — | classe de taille changée (`event.value()` : la classe) ; aussi au premier affichage |

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
| `event.instance()` → `string` | `id` de l'instance du composant d'où vient une action propre (`ui.component(…).on`), `""` sinon |

## État des nœuds

| Méthode | Rôle |
|---------|------|
| `ui.get(id, prop)` → `mixed` | valeur courante : saisie de l'utilisateur, valeur posée par `set`, sinon celle du `.oc.ui`, sinon `null` |
| `ui.set(id, prop, value)` | change une propriété (`value`, `text`, `checked`, `visible`, `enabled`, `suggestions` d'un champ…) ; le nœud est réaffiché (sauf `suggestions` : le menu seul) |
| `ui.setItems(id, rows)` | éléments d'une `list` (`array<map<string, mixed>>`, lus par `bind`) |
| `ui.has(id)` → `bool` | le nœud `id` est-il dans la variante de taille affichée (gabarits `item` des listes compris) ? `false` s'il n'est que dans une autre variante |

`id` doit exister dans le `.oc.ui` (sinon `UIException` 102). L'état est
gardé par le runtime : `get` ne questionne pas l'interface, et l'état posé
avant `run()` est affiché dès l'ouverture.
Les valeurs texte de `set` sont remplies par les variables `${nom}`.

## Liaisons

Un nœud lié par `bind: user.name` hors d'une liste ([oc-ui.md](../oc-ui.md#liaisons-à-un-objet))
affiche et écrit un champ de l'objet relié à la racine `user` :

```ocara
class Profile {
    public property name:string = "Ana"
    public property age:int = 30
}

const profile:Profile = use Profile()
ui.bind("user", profile)
ui.on("older", nameless(u:UI, event:UIEvent): void {
    profile.age = profile.age + 1   // l'interface affiche 31 après le gestionnaire
})
```

| Méthode | Rôle |
|---------|------|
| `ui.bind(root, object)` | relie la racine `root` à `object` (instance d'une classe) et affiche ses champs ; un nouveau `bind` sur la racine remplace l'objet |
| `ui.unbind(root)` | délie la racine ; les nœuds gardent leurs dernières valeurs |
| `ui.refresh()` | relit les objets liés, après une modification hors d'un gestionnaire (thread, timer) |

- **Quand l'interface suit le code** : au `bind`, après chaque gestionnaire
  d'action (tous ceux de l'action) et sur `ui.refresh()`. Seuls les nœuds
  dont la valeur a changé sont réaffichés.
- **Quand le code suit l'interface** : une saisie est écrite dans l'objet sur
  le thread des gestionnaires, avant l'action suivante, convertie au type du
  champ. Une saisie invalide n'écrit rien. La valeur écrite ne revient pas au
  moteur (le curseur reste en place).
- **Thread** : l'objet n'est lu et écrit que sur le thread des
  gestionnaires, et lors du `bind` sur le thread qui l'appelle. Un objet
  modifié depuis un autre thread s'affiche au `ui.refresh()` suivant.
- **Vérifications** : le compilateur génère l'accès aux champs de la classe
  (pas de réflexion à l'exécution). Sur une interface créée par `use UI(…)`
  dans la même fonction, chaque chemin lié du `.oc.ui` est vérifié dès la
  compilation : champ existant, accessible depuis l'appel, de type
  compatible avec la propriété ([E74](../diagnostics.md#e74--liaison-ui-invalide)).
  Ailleurs (gestionnaire qui reçoit `ui:UI`), au `bind` : `UIException` 107.
- **Mémoire** : l'interface retient l'objet lié jusqu'à `unbind`, un nouveau
  `bind` ou sa propre libération ; un objet qui garde `ui` dans un champ
  forme un cycle repris par le collecteur.

## Formulaires

Champs `required`, conteneur `form: true`, bouton `submit: true` et action
`onInvalid` : voir [oc-ui.md](../oc-ui.md#formulaires). Une action qui
soumet un formulaire incomplet n'arrive pas aux gestionnaires ; `onInvalid`
reçoit la liste des champs vides :

```ocara
ui.on("missing", nameless(u:UI, event:UIEvent): void {
    u.toast(`Champs à remplir : ${event.value()}`)
})
```

| Méthode | Rôle |
|---------|------|
| `ui.validate(container)` → `array<string>` | `id` des champs obligatoires vides du conteneur `container` (sans les marquer `invalid`) ; `UIException` 102 si l'`id` est absent |

## Animations

Animations déclarées sous `animations:` et posées par `animation:` : voir
[oc-ui.md](../oc-ui.md#animations).

| Méthode | Rôle |
|---------|------|
| `ui.animate(id, name, options)` | relance l'animation `name` sur le nœud `id` ; `options` (facultatif, `map<string, mixed>`) : `duration`, `delay`, `easing`, `iterations`, `direction`, `fill` |
| `ui.stopAnimation(id)` | arrête l'animation du nœud |

`id` absent : `UIException` 102 ; animation absente : 108. Avec des
littéraux, les deux sont vérifiés dès la compilation.

## Navigation et cycle de vie

| Méthode | Rôle |
|---------|------|
| `ui.setTheme(theme)` | thème affiché, `auto` (celui du système), `light` ou `dark`, à la place de `window.theme` ; appliqué à chaud (écran et état gardés), ou dès l'affichage s'il est appelé avant `run()` ; `UIException` 109 pour un autre nom |
| `ui.navigate(screen)` | affiche l'écran (`UIException` 103 s'il n'est pas déclaré) ; un écran déjà ouvert : retour à lui ; l'écran affiché : son `onShow` relancé |
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
| `ui.pickFile(extensions)` → `string` | sélecteur natif, `""` si annulé | sélecteur de documents du système ; le document est copié dans le cache de l'app, chemin rendu ; `""` si annulé |
| `ui.saveFile(name)` → `string` | idem | `UIException` 104 : pas de chemin de fichier écrivable hors de l'app (enregistrer avec [AndroidFiles](AndroidFiles.md)`::saveToDownloads`) |
| `ui.notify(title, text)` | notification système | notification (permission `notifications` requise, voir [AndroidNotifications](AndroidNotifications.md)) |

## Vue web

Le nœud `webview` (`id` obligatoire) est une **vraie vue web isolée**, sans
`<iframe>` : WebKit posé sur la fenêtre au rectangle du nœud (desktop
Linux), `WebView` d'Android. Contenu : `url`, `file` (relatif au dossier du
programme) ou `html` ; `ui.set(id, "url", …)` recharge.

| Méthode / événement | Rôle |
|---------------------|------|
| `onMessage` (`.oc.ui`) | la page a appelé `window.ocara.post(data)` ; `event.value()` : la donnée (JSON décodé) |
| `onLoad`, `onError` | page chargée (`value` : son URL), échec (`{url, message}`) ; `ui.get(id, "url")`, `ui.get(id, "canGoBack")` suivent la page |
| `ui.postMessage(id, data)` | `window.ocara.onMessage(callback)` dans la page |
| `ui.eval(id, script)` → `string` | résultat du script (JSON pour un objet), attendu |
| `ui.webBack(id)`, `ui.webForward(id)`, `ui.reload(id)` | historique de la vue |
| `allow` (`.oc.ui`) | hôtes permis ; un lien hors liste s'ouvre dans le navigateur du système |

Sur Android, le bouton retour remonte d'abord l'historique de la vue web.
Limites actuelles : pas encore de vue posée sous Windows et macOS (la place
du nœud reste vide) ; sur desktop, une fenêtre `decorations: false` qui
contient une vue web ne se redimensionne plus par ses bords ; sur Android,
pour une `url`, `window.ocara` est disponible dès le début du chargement
(un script de la page exécuté plus tôt peut appeler `ocaraNative.post`).

## Composants d'écran

`fab`, `drawer` et `bottomSheet` (bouton flottant, tiroir de navigation,
panneau inférieur), `floating` (conteneur posé en `x`/`y`/`z`) et
`spinner` (indicateur d'attente) sont décrits dans le `.oc.ui` et rendus
sur les deux plateformes ; tiroir et panneau s'ouvrent par
`ui.set(id, "open", true)`, se ferment par `ui.set(id, "open", false)` ou
par l'utilisateur (action `onDismiss`) ; `ui.set(id, "x", 120)` déplace un
`floating`. Voir [docs/oc-ui.md](../oc-ui.md), « Composants d'écran ».

## Spécificités de plateforme

`ui.tauri()` → `Tauri` (desktop, `import ocara.Tauri`) : la fenêtre de
l'interface, pour ce qu'`ocara.UI` ne standardise pas (toujours au premier
plan, zone de notification, raccourcis globaux…). Sur Android :
`UIException` 104 ; les spécificités passent par les méthodes statiques
d'[AndroidUI](AndroidUI.md).

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
| 104 | service indisponible sur cette plateforme (`ui.tauri()` et `saveFile` sur Android) |
| 105 | arbre d'interface invalide (binaire altéré) |
| 106 | composant absent du `.oc.ui` (`ui.component`) |
| 107 | `ui.bind` : racine absente des liaisons du `.oc.ui`, chemin lié absent de l'objet (ou inaccessible), ou de type incompatible |
| 108 | `ui.animate` : animation absente du `.oc.ui` |
| 109 | `ui.setTheme` : thème inconnu (attendu : `auto`, `light`, `dark`) |

## Voir aussi

- [docs/oc-ui.md](../oc-ui.md) — format `.oc.ui`, aperçus, moteurs de rendu
- [UIComponent](UIComponent.md) — actions propres et instances d'un composant (`ui.component`)
- [Tauri](Tauri.md), [AndroidUI](AndroidUI.md)
- [Garage Ocara natif](../../examples/advanced/mini_project_ui_hexa/README.md) — application complète en architecture hexagonale (desktop et Android)
