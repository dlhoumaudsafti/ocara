# Fichiers `.oc.ui` : interfaces décrites en YAML

Un `.oc.ui` décrit l'interface d'une application pour le builtin `ocara.UI` :
les fenêtres, les écrans, les nœuds (textes, champs, boutons, listes, vue
web…) et les **actions** que déclenchent leurs événements. Le même fichier
sert le desktop (Tauri) et Android (Compose) ; le code Ocara lie chaque
action à une méthode (`ui.on("search", GarageController::search)`).

Le fichier est lu **à la compilation** (`use UI("x.oc.ui")`). Une erreur (YAML invalide, type ou
propriété inconnue, valeur hors de son domaine, `id` en double, style non
déclaré…) est une erreur de compilation, située à la ligne et à la colonne
du `.oc.ui`. Le compilateur en tire un arbre JSON normalisé, embarqué dans
le binaire et interprété par le moteur de rendu de la plateforme.

> Un programme affiche un `.oc.ui` avec le builtin [`ocara.UI`](builtins/UI.md)
> (`use UI("x.oc.ui")`, `ui.on`, `get`/`set`).

## Vérifier un fichier

```sh
ocara examples/ui/garage.oc.ui          # check ok — 3 id, 3 action(s).
ocara examples/ui/garage.oc.ui --dump   # arbre JSON normalisé
```

Les erreurs ont le format habituel du compilateur :

```text
garage.oc.ui:10:19: error: type de nœud 'buton' inconnu (vouliez-vous dire 'button' ?)
garage.oc.ui:12:29: error: id 'name' déjà utilisé ligne 11
```

## Aperçus

**Desktop** : `--preview` écrit une page HTML autonome qui affiche
l'interface avec le moteur de la fenêtre Tauri ; les événements s'affichent
en bas de page.

```sh
ocara examples/ui/components.oc.ui --preview            # → /tmp/components.preview.html
ocara examples/ui/components.oc.ui --preview -o ui.html
```

**Android** (build debug de `packaging/android`) : l'arbre produit par
`--dump`, poussé dans le dossier de l'app, s'affiche à la place du contenu ;
les événements vont dans `adb logcat -s ocara.UI`.

```sh
ocara examples/ui/components.oc.ui --dump > components.json
adb push components.json /sdcard/Android/data/com.ocara.demo/files/
adb shell am start -n com.ocara.demo/.MainActivity --es ocara.ui.preview components.json
```

Le fichier poussé peut aussi être `{ "tree": <arbre>, "screen": "list",
"set": [["people", "items", [...]]] }` pour ouvrir un écran avec un état.

## Exemple

```yaml
window:
  title: "Garage"
  width: 900
  height: 640
  theme: auto

styles:
  titre: { fontSize: 22, bold: true }

start: home
screens:
  home:
    layout:
      type: column
      spacing: 12
      padding: 16
      children:
        - { type: heading, value: "Mes voitures", style: titre }
        - type: row
          spacing: 8
          children:
            - { type: input, id: search, placeholder: "Rechercher", onSubmit: search, width: fill }
            - { type: iconButton, icon: search, label: "Rechercher", onClick: search }
        - type: list
          id: cars
          onSelect: open_car
          item:
            type: row
            children:
              - { type: text, bind: brand }
              - { type: text, bind: model }
  details:
    title: "Détail"
    layout:
      type: column
      children:
        - { type: text, id: car_title, style: titre }
        - { type: button, text: "Retour", onClick: back }
```

## Racine

| Clé | Contenu |
|-----|---------|
| `window` | fenêtre principale (ci-dessous) |
| `styles` | styles nommés : `nom: { propriétés de présentation }` |
| `screens` | écrans nommés : `nom: { title, layout }` |
| `start` | écran affiché au lancement ; facultatif s'il n'y a qu'un écran |
| `layout` | à la place de `screens` : un seul écran, nommé `main` |
| `menu` | menu de l'application (entrées décrites plus bas) |

### `window`

| Clé | Valeur |
|-----|--------|
| `title` | texte |
| `width`, `height`, `minWidth`, `minHeight` | entier, en pixels logiques (ignorés sur Android) |
| `resizable` | booléen |
| `theme` | `auto`, `light`, `dark` |
| `primary` | couleur principale |
| `tauri` | `decorations`, `alwaysOnTop`, `transparent`, `center`, `devtools` (booléens) |
| `android` | `orientation` (`portrait`, `landscape`, `auto`), `fullscreen`, `keepScreenOn`, `systemBars` (`light`, `dark`) |

## Nœuds

Un nœud est une table dont `type` est l'un des types ci-dessous.

| Type | Propriétés propres | Obligatoires |
|------|--------------------|--------------|
| `column`, `row`, `stack` | `children`, `spacing`, `scroll` | |
| `grid` | `children`, `spacing`, `scroll`, `columns` (1 à 64) | |
| `card` | `children`, `spacing`, `onClick` | |
| `divider`, `spacer` | | |
| `text` | `value`, `bind` | |
| `heading` | `value`, `bind`, `level` (1 à 3) | |
| `image` | `src`, `bind`, `fit` (`contain`, `cover`, `fill`, `none`) | `src` ou `bind`, `label` |
| `icon` | `name` | `name` |
| `progress` | `value` (0 à 1 ; absente : indéterminée) | |
| `list` | `item` (gabarit d'un élément), `spacing`, `onSelect` | `item` |
| `webview` | une source parmi `url`, `file`, `html` ; `javascript`, `storage`, `allow` (liste d'hôtes), `zoom`, `userAgent`, `webHandlers` (nom → action), `onLoad`, `onError`, `onMessage` | |
| `input` | `value`, `bind`, `placeholder`, `kind` (`text`, `password`, `email`, `number`, `multiline`), `readonly`, `onChange`, `onSubmit`, `onFocus`, `onBlur` | |
| `checkbox`, `switch` | `checked`, `bind`, `onChange` | |
| `radio`, `select` | `options`, `value`, `onChange` | `options` |
| `slider` | `min` (< `max`), `max`, `step`, `value`, `onChange` | |
| `date`, `time` | `value` (ISO), `onChange` | |
| `button` | `text`, `icon`, `variant` (`primary`, `secondary`, `text`), `onClick` | |
| `iconButton` | `icon`, `onClick` | `icon`, `label` |

Propriétés communes à tous les nœuds :

| Propriété | Valeur |
|-----------|--------|
| `id` | identifiant (lettres, chiffres, `_`), unique dans le fichier ; seuls les nœuds qui en ont un sont accessibles depuis Ocara |
| `style` | nom d'un style déclaré sous `styles` |
| `visible`, `enabled` | booléen |
| `width`, `height` | nombre, `fill` ou `wrap` |
| `weight` | nombre (part de l'espace libre) |
| `align` | `start`, `center`, `end`, `stretch` |
| `label` | texte lu par l'accessibilité |
| `tooltip` | texte |
| `contextMenu` | entrées de menu (clic droit, appui long) |
| `tauri`, `android` | surcharge de propriétés pour une plateforme (voir plus bas) |
| présentation | `fontSize`, `bold`, `italic`, `color`, `background`, `padding`, `margin`, `radius`, `border`, `elevation` (aussi permises dans `styles`) |

### Tailles par défaut

Les deux moteurs suivent les mêmes règles :

- dans une colonne, les champs (`input`, `select`, `slider`, `date`,
  `time`, `radio`), `list`, `card`, `divider`, `progress`, `webview` et les
  conteneurs prennent toute la largeur ; textes, titres, boutons, cases,
  icônes et images gardent leur taille ;
- dans une ligne, chaque enfant garde sa taille ; s'il manque de la place,
  textes, cases et champs se la partagent (boutons, icônes et images ne
  rétrécissent pas) ;
- `spacer` prend l'espace libre ; `width: fill` (ligne) ou `height: fill`
  (colonne) aussi ; `weight` le partage.

### Icônes

`icon.name`, `button.icon` et `iconButton.icon` nomment une icône du jeu
commun (Material Icons) : `add`, `back`, `check`, `close`, `delete`,
`edit`, `favorite`, `forward`, `home`, `info`, `menu`, `more`, `person`,
`refresh`, `search`, `share`, `star`. Le dessin des icônes utilisées est
copié dans l'arbre (`icons`) : il est le même sur les deux plateformes.

### Valeurs

- **Longueurs** : nombres en pixels logiques (px CSS sur desktop, dp sur
  Android). `padding`/`margin` : un nombre, ou 2 (vertical, horizontal) ou 4
  (haut, droite, bas, gauche).
- **Couleurs** : `"#rrggbb"` ou `"#rrggbbaa"`, **entre guillemets** : en
  YAML, `#` précédé d'une espace commence un commentaire.
- **Textes** : un nombre ou un booléen donné à une propriété texte est
  converti en texte (`value: 42` → `"42"`).
- **Choix** (`options`) : `[a, b]` ou `[{ value: a, text: "Libellé" }]`,
  normalisés en `{ value, text }`.

### Listes et `bind`

`item` est le gabarit d'un élément de la liste. Dans ce gabarit, `bind:
champ` affiche le champ de chaque élément fourni par `ui.setItems`
(`bind: user.name` pour un champ imbriqué). Hors d'un gabarit `item`, `bind`
est une erreur.

### Surcharges de plateforme

`tauri:` et `android:` reprennent les propriétés du nœud, pour cette
plateforme seulement :

```yaml
- { type: button, text: "Envoyer", variant: primary, android: { variant: text } }
```

`type`, `id`, `children` et `item` ne se surchargent pas.

## Actions

Les événements (`onClick`, `onChange`, `onSubmit`, `onSelect`, `onFocus`,
`onBlur`, `onLoad`, `onError`, `onMessage`), les `webHandlers` et le
`action` des entrées de menu nomment une **action** (lettres, chiffres,
`_`). Le code Ocara la lie par `ui.on(action, Classe::méthode)`. Les actions
`back` et `close` sont prises en charge par le moteur sans `ui.on`.

## Menus

`menu` et `contextMenu` sont des listes d'entrées : `text`, `action`,
`shortcut` (`"CmdOrCtrl+S"`), `enabled`, `checked` (entrée cochable),
`separator: true`, `items` (sous-menu). Une entrée a un `text`, sauf un
séparateur.

## Limites du YAML accepté

Un seul document par fichier ; pas d'ancres ni d'alias (`&nom`, `*nom`) ;
une clé en double dans une table est une erreur.

## Arbre normalisé

`--dump` affiche ce que le binaire embarque : `window`, `styles`, `start`,
`screens` (chaque écran : `title`, `layout`), `menu`, `icons`, plus `ids`
(chaque `id` et le type de son nœud) et `actions` (liste triée), qui
servent à vérifier les `ui.on` et les `get`/`set`.

## Moteurs de rendu

| Plateforme | Moteur | Fichiers |
|------------|--------|----------|
| Desktop | DOM dans la vue de la fenêtre Tauri, sans framework | `runtime_tauri/src/ui/engine.js`, `engine.css` |
| Android | Compose (Material 3) dans l'Activity | `packaging/android/…/bridge/OcaraUIEngine.kt`, `OcaraUINodes.kt` |

Contrat commun :

- `start(arbre)` / `show(arbre)` affiche l'écran `start` ; `set(id, prop,
  valeur)` change l'état d'un nœud (`value`, `text`, `checked`, `visible`,
  `enabled`, `items` d'une liste…) et le réaffiche ; `navigate(écran)`,
  `back()` ;
- l'état d'un nœud survit au changement d'écran ;
- chaque valeur changée par l'utilisateur part en `{ id, prop, value }`
  (l'état côté Ocara reste à jour, `get` n'a pas à interroger l'interface) ;
  chaque action déclarée en `{ action, id, value, index }` (`index` pour
  `onSelect`, `value` : l'élément de la liste) ; `ready` à l'affichage ;
- `back` est traité par le moteur (écran précédent, bouton retour Android) ;
  sur l'écran de départ, il remonte en action `back`.

Les tests du moteur desktop (`ci/ui_engine.sh`, Chrome sans écran) font
partie de `make regression`.

## Voir aussi

- [Fiche `ocara.UI`](roadmap.d/langage-builtin-ui-multiplateforme.md) : principe, inventaire, API Ocara
- [Tauri](builtins/Tauri.md), [AndroidUI](builtins/AndroidUI.md) : spécificités de chaque plateforme
- Code : `src/core/ui_schema.d/` (lecteur YAML positionné, tables du schéma, icônes, validation, aperçu)
