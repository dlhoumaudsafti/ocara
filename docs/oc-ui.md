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
| `styles` | styles nommés : `nom: { propriétés de présentation }` (aussi dans les fichiers inclus, voir « Écrans inclus ») ; `dark:` et `light:` y surchargent des styles pour un thème (voir « Styles par thème ») |
| `screens` | écrans nommés : `nom: { title, layout, onShow }` (`onShow` : action lancée à chaque affichage de l'écran, valeur : son nom) |
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
| `list` | `item` (gabarit d'un élément), `spacing`, `onSelect` ; `display` (`list`, `grid`) et, en grille, `itemWidth`, `minColumns`, `maxColumns` (1 à 64), `direction` (`ltr`, `rtl`, `ltr-snake`, `rtl-snake`) | `item` |
| `webview` | une source parmi `url`, `file`, `html` ; `javascript`, `storage`, `allow` (liste d'hôtes), `zoom`, `userAgent`, `webHandlers` (nom → action), `onLoad`, `onError`, `onMessage` | |
| `input` | `value`, `bind`, `placeholder`, `kind` (`text`, `password`, `email`, `number`, `multiline`), `readonly`, `onChange`, `onSubmit`, `onFocus`, `onBlur` | |
| `checkbox`, `switch` | `checked`, `bind`, `onChange` | |
| `radio`, `select` | `options`, `value`, `onChange` | `options` |
| `slider` | `min` (< `max`), `max`, `step`, `value`, `onChange` | |
| `date`, `time` | `value` (ISO), `placeholder`, `onChange` | |
| `button` | `text`, `icon`, `variant` (`primary`, `secondary`, `text`), `onClick` | |
| `iconButton` | `icon`, `onClick` | `icon`, `label` |
| `android.fab` | `icon`, `text` (bouton étendu), `onClick` | `icon`, `label` |
| `android.bottomSheet` | `children`, `spacing`, `open`, `onDismiss` | `id` |
| `android.drawer` | `children`, `spacing`, `open`, `onDismiss` | `id` |

Propriétés communes à tous les nœuds :

| Propriété | Valeur |
|-----------|--------|
| `binds` | dans un gabarit `item` : `{ propriété: champ }`, propriétés lues dans l'élément (voir plus bas) |
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

### Composants propres à Android

Les types préfixés `android.` sont des composants Material sans
équivalent desktop ; ils sont validés partout et **ignorés sur desktop**.
Déclarés n'importe où dans la disposition d'un écran, ils sont placés là
où Material les attend :

- `android.fab` : bouton flottant de l'écran (le premier compte) ; avec
  `text`, bouton étendu (icône et texte) ;
- `android.drawer` : tiroir de navigation de l'écran (le premier compte),
  ouvert par l'icône de menu de la barre de titre ou par
  `ui.set(id, "open", true)` ;
- `android.bottomSheet` : panneau inférieur, ouvert par
  `ui.set(id, "open", true)`.

Fermé par l'utilisateur (geste, toucher à côté), un tiroir ou un panneau
repasse à `open: false` (lu par `ui.get`) et déclenche `onDismiss`.

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

### Images embarquées

Le `src` local d'une `image` (relatif au `.oc.ui` ; png, jpg, gif, webp,
svg ; 4 Mo au plus) est lu à la compilation et embarqué dans l'arbre (URI
`data:`) : le binaire, APK compris, n'a aucun fichier à livrer à côté. Un
fichier absent est une erreur de compilation. Une URL `https://…` reste
telle quelle (chargée par le desktop seulement). Le SVG n'est pas affiché
sur Android.

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

`binds` fait de même pour n'importe quelle propriété du nœud :

```yaml
item:
  type: card
  children:
    - { type: text, bind: reference }
    - { type: text, bind: estimated, binds: { color: badge_color, visible: has_estimate } }
    - { type: iconButton, icon: delete, label: "Supprimer", onClick: delete_car }
```

Un `onClick` d'un nœud placé dans un élément de liste porte l'élément
(`value`) et son rang (`index`), comme `onSelect`, sans déclencher
l'`onSelect` de la liste.

### Surcharges de plateforme

`tauri:` et `android:` reprennent les propriétés du nœud, pour cette
plateforme seulement :

```yaml
- { type: button, text: "Envoyer", variant: primary, android: { variant: text } }
```

`type`, `id`, `children` et `item` ne se surchargent pas.

### Liste en grille

Avec `display: grid`, les éléments d'une liste se suivent sur une ligne et
passent à la ligne suivante quand la place manque :

```yaml
- type: list
  id: cars
  display: grid
  spacing: 8
  minColumns: 1        # facultatif
  maxColumns: 4        # facultatif
  itemWidth: 280       # facultatif
  direction: ltr       # ltr (défaut), rtl, ltr-snake, rtl-snake
  item: { … }
```

- Sans `itemWidth`, chaque case garde sa **largeur naturelle** ;
  `maxColumns` met au plus ce nombre de cases par ligne (largeur minimale
  d'une case : la ligne divisée par `maxColumns`), `minColumns` au moins
  (largeur maximale : la ligne divisée par `minColumns`).
- Avec `itemWidth`, le nombre de colonnes suit la largeur de la liste :
  `(largeur + spacing) / (itemWidth + spacing)`, borné par `minColumns` et
  `maxColumns` ; les cases se partagent la ligne. Il est recalculé à chaque
  changement de largeur (fenêtre redimensionnée, rotation, écran plié).
- `direction` : de gauche à droite (`ltr`), de droite à gauche (`rtl`), ou
  en **serpentin**, chaque ligne repartant du côté où la précédente s'arrête.
  Le serpentin a besoin d'un nombre fixe de colonnes : sans `itemWidth`, il
  prend `itemWidth: 50`. Avec 3 colonnes et 7 éléments :

| `direction` | ligne 1 | ligne 2 | ligne 3 |
|---|---|---|---|
| `ltr` | 1 2 3 | 4 5 6 | 7 · · |
| `rtl` | 3 2 1 | 6 5 4 | · · 7 |
| `ltr-snake` | 1 2 3 | 6 5 4 | 7 · · |
| `rtl-snake` | 3 2 1 | 4 5 6 | · · 7 |

Le rang d'un élément (`onSelect`, `onClick` du gabarit) reste celui des
données, pas sa place à l'écran. Les options de grille sans `display:
grid`, ou `minColumns` supérieur à `maxColumns`, sont des erreurs.

### Styles par thème

Sous `styles:`, `dark:` et `light:` surchargent des styles pour un thème :
leurs propriétés s'appliquent par-dessus le style de même nom quand ce
thème est affiché (`window.theme`, ou celui du système pour `auto`).

```yaml
styles:
  maClass: { bold: true, color: "#1565c0" }    # tous les thèmes
  dark:
    maClass: { color: "#90caf9" }               # thème sombre : par-dessus maClass
```

`dark` et `light` sont donc des noms réservés sous `styles:`. Les
surcharges peuvent aussi être déclarées dans un fichier inclus ; un même
style surchargé deux fois pour un thème est une erreur. Dans l'arbre
normalisé, elles sont sous `themeStyles` (`{ "dark": {…}, "light": {…} }`).

## Écrans inclus

`screens:` peut lister d'autres fichiers sous la clé `includes` (chemins
relatifs au fichier qui les inclut) ; chacun contient `screens:` et, s'il
le faut, les `styles:` de ses écrans, et peut lui-même en inclure d'autres :

```yaml
# garage.oc.ui
styles:
  muted: { color: "#757575" }
screens:
  form:
    title: "Formulaire"
    layout: { type: column, children: [] }
  includes:
    - ui.d/list.oc.ui
    - ui.d/other.oc.ui
```

```yaml
# ui.d/list.oc.ui : les écrans et leurs styles ensemble
styles:
  row_title: { fontSize: 16, bold: true }
screens:
  list:
    layout:
      type: column
      children:
        - { type: text, value: "Voitures", style: row_title }
        - { type: text, value: "aucune", style: muted }
```

Les écrans inclus partagent les `id` (uniques dans toute l'interface) et
les actions du fichier principal. Les styles forment un seul espace de
noms : un style déclaré dans un fichier (principal ou inclus) sert dans
tous les écrans, quel que soit l'ordre des inclusions, et un style déclaré
deux fois, même dans deux fichiers, est une erreur. Une erreur est située
dans le fichier inclus ; un écran déclaré deux fois, ou un fichier inclus
deux fois (inclusion circulaire comprise), est une erreur.

## Variables

Tout texte peut contenir des variables `${nom}` (lettres, chiffres, `_`),
remplies à l'exécution par la map passée à `use UI(path, vars)` et
changées par `ui.setVars(vars)` : textes de langue, nom de l'utilisateur…

```yaml
- { type: heading, value: "${cars_title}" }
- { type: input, id: search, placeholder: "${search}" }
```

Les valeurs passées à `ui.set` et les textes de `toast`, `alert`,
`confirm` et `notify` sont remplis de la même façon :
`ui.set("flash", "value", "${flash_saved}")` affiche le message dans la
langue courante.

Une variable sans valeur reste affichée telle quelle (`${nom}`) et un
avertissement l'indique ; `$${` écrit un `${` littéral. Les variables
utilisées sont listées par `ocara x.oc.ui` et dans l'arbre (`vars`).

## Tailles d'écran

Une interface peut avoir des **variantes** pour les fenêtres ou écrans plus
larges, à côté du fichier principal, qui reste le défaut :

```text
garage.oc.ui        défaut
garage.sm.oc.ui     à partir de 600
garage.md.oc.ui     à partir de 840
```

La largeur disponible, en pixels logiques (px CSS sur desktop, dp sur
Android, indépendants de la densité de l'écran), donne une **classe** ; le
moteur affiche la plus grande variante dont le seuil est atteint, sinon le
défaut. Seuils (largeur minimale), calés sur les classes de taille de
Material 3 :

| Classe | À partir de | Typiquement |
|--------|-------------|-------------|
| `xxs` | 0 | montre, très petit écran |
| `xs` | 360 | téléphone en portrait |
| `sm` | 600 | grand téléphone en paysage, petite tablette, pliable ouvert |
| `md` | 840 | tablette en paysage, petite fenêtre de bureau |
| `lg` | 1200 | fenêtre de bureau |
| `xl` | 1600 | grand écran |
| `xxl` | 1920 | très grand écran |

Les variantes sont validées et embarquées à la compilation avec le fichier
principal ; un même `id` y a le même type (l'état d'un nœud passe d'une
variante à l'autre). Le changement se fait **à la volée** : fenêtre
redimensionnée, rotation, écran plié ou déplié, état et écran courant
gardés (un écran absent de la variante affichée fait revenir à `start`, et
`ui.navigate` vers lui ne change rien). Côté Ocara, `ui.sizeClass()` donne
la classe courante, et l'action `resize` (valeur : la classe) prévient d'un
changement.

Une variante est un `.oc.ui` complet, vérifiable seul
(`ocara garage.md.oc.ui`) ; elle peut inclure les mêmes écrans que le
défaut (`screens.includes`).

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

## Éditeur

`ocara --ui-schema` produit le schéma JSON des `.oc.ui` depuis les tables
du compilateur ; l'extension VS Code d'Ocara l'associe aux fichiers `.oc.ui`
(complétion et validation à la frappe avec l'extension YAML de Red Hat,
`redhat.vscode-yaml`). La validation qui fait foi reste celle du
compilateur (styles déclarés, `id` uniques, `bind`, images).

Le bouton en haut à droite d'un `.oc.ui` ouvre le **designer** de
l'extension (comme l'aperçu Markdown) : aperçu de l'écran avec l'apparence
du moteur desktop, composants glissés depuis une palette, structure des
nœuds, panneau de toutes les propriétés, styles, menu et fenêtre. Il
modifie le même document que l'éditeur texte (annulable, seules les lignes
touchées changent) et affiche les erreurs du compilateur sur les nœuds
(`tools/highlight/vsode/README.md`).

## Limites du YAML accepté

Un seul document par fichier ; pas d'ancres ni d'alias (`&nom`, `*nom`) ;
une clé en double dans une table est une erreur.

## Arbre normalisé

`--dump` affiche ce que le binaire embarque : `window`, `styles`, `themeStyles`, `start`,
`screens` (chaque écran : `title`, `layout`), `menu`, `icons`, `vars`,
`sizes` (seuils des classes), `variants` (arbres des variantes de taille),
`source` (build de debug : chemins pour le rechargement à chaud), plus `ids`
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
- `navigate` vers un écran déjà ouvert plus tôt y revient (les écrans
  ouverts après lui sont retirés) ; vers l'écran affiché, il relance son
  `onShow` (rechargement des données) ;
- `back` est traité par le moteur (écran précédent, bouton retour Android) ;
  sur l'écran de départ, il remonte en action `back`.

`OCARA_UI_TRACE=1` affiche sur la sortie d'erreur chaque message reçu du
moteur (desktop) ; sur Android, ils sont dans `adb logcat -s ocara.UI`.

Les tests du moteur desktop (`ci/ui_engine.sh`, Chrome sans écran) font
partie de `make regression`.

## Voir aussi

- [Fiche `ocara.UI`](roadmap.d/langage-builtin-ui-multiplateforme.md) : principe, inventaire, API Ocara
- [Tauri](builtins/Tauri.md), [AndroidUI](builtins/AndroidUI.md) : spécificités de chaque plateforme
- Code : `src/core/ui_schema.d/` (lecteur YAML positionné, tables du schéma, icônes, validation, aperçu)
