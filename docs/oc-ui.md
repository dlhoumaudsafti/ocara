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
| `components` | composants déclarés : `nom: { params, layout }`, `includes` (voir « Composants ») |
| `styles` | styles nommés : `nom: { propriétés de présentation }` (aussi dans les fichiers inclus, voir « Écrans inclus ») ; `dark:` et `light:` y surchargent des styles pour un thème (voir « Styles par thème ») |
| `animations` | animations nommées, sur le modèle des `@keyframes` CSS (voir « Animations ») |
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
| `column`, `row`, `stack` | `children`, `spacing`, `scroll`, `form` | |
| `grid` | `children`, `spacing`, `scroll`, `columns` (1 à 64), `form` | |
| `card` | `children`, `spacing`, `onClick`, `form` | |
| `divider`, `spacer` | | |
| `text` | `value`, `bind` | |
| `heading` | `value`, `bind`, `level` (1 à 3) | |
| `image` | `src`, `bind`, `fit` (`contain`, `cover`, `fill`, `none`) | `src` ou `bind`, `label` |
| `icon` | `name` | `name` |
| `progress` | `value` (0 à 1 ; absente : indéterminée) | |
| `list` | `item` (gabarit d'un élément), `spacing`, `onSelect` ; `display` (`list`, `grid`) et, en grille, `itemWidth`, `minColumns`, `maxColumns` (1 à 64), `direction` (`ltr`, `rtl`, `ltr-snake`, `rtl-snake`) | `item` |
| `webview` | une source parmi `url`, `file`, `html` ; `javascript`, `storage`, `allow` (liste d'hôtes), `zoom`, `userAgent`, `webHandlers` (nom → action), `onLoad`, `onError`, `onMessage` | |
| `input` | `value`, `bind`, `placeholder`, `kind` (`text`, `password`, `email`, `number`, `multiline`), `readonly`, `required`, `onChange`, `onSubmit` | |
| `checkbox`, `switch` | `checked`, `bind`, `required`, `onChange` | |
| `radio`, `select` | `options`, `value`, `required`, `onChange` | `options` |
| `slider` | `min` (< `max`), `max`, `step`, `value`, `onChange` | |
| `date`, `time` | `value` (ISO), `placeholder`, `required`, `onChange` | |
| `button` | `text`, `icon`, `variant` (`primary`, `secondary`, `text`), `onClick`, `submit`, `onInvalid` | |
| `iconButton` | `icon`, `onClick`, `submit`, `onInvalid` | `icon`, `label` |
| `link` | `text`, `onClick` ; état `visited` après le premier clic (voir « Styles d'état et effets ») | `text` |
| `fab` | `icon`, `text` (bouton étendu), `position` (voir « Composants d'écran »), `onClick` | `icon`, `label` |
| `bottomSheet` | `children`, `spacing`, `open`, `onDismiss` | `id` |
| `drawer` | `children`, `spacing`, `open`, `side` (`start`, `end`), `permanentFrom` (classe de taille), `onDismiss` | `id` |
| `floating` | `children`, `spacing`, `position` (`fixed`, `absolute`), `anchor`, `x`, `y` | |
| `spinner` | `kind` (`circle`, `dots`, `bars`, `pulse`, `ring`, `orbit`) ou `src` (image animée), `size` (4 à 1000) | |

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
| `z` | entier de -1000 à 1000 : ordre d'empilement (le plus grand devant) |
| `label` | texte lu par l'accessibilité |
| `tooltip` | texte |
| `contextMenu` | entrées de menu (clic droit, appui long) |
| événements | `onHover`, `onLeave`, `onFocus`, `onBlur`, `onLongPress`, `onDoubleClick`, `onSwipe` (voir [Événements](#événements)) |
| `tauri`, `android` | surcharge de propriétés pour une plateforme (voir plus bas) |
| présentation | `fontSize`, `bold`, `italic`, `color`, `background`, `padding`, `margin`, `radius`, `border`, `elevation`, `opacity`, `blur`, `backdropBlur`, `shadow`, `transition`, `states` (tables d'état `hover`, `focus`, `active`, `disabled`, `invalid`, `checked`, `visited`) — aussi permises dans `styles`, voir « Styles d'état et effets » |
| `animation`, `onAnimationEnd` | animation du nœud et action lancée à sa fin (voir « Animations ») |

### Composants d'écran

`fab`, `drawer`, `bottomSheet` et `floating` fixe sont rendus pareil sur
desktop et sur Android. Déclarés n'importe où dans la disposition d'un
écran, ils sont placés dans une couche au-dessus de lui (rien à leur place
dans l'arbre) :

- `fab` : bouton flottant de l'écran (le premier compte), à 16 px du bord ;
  avec `text`, bouton étendu (icône et texte). `position` : `bottomEnd`
  (défaut), `bottomCenter`, `bottomStart`, `topEnd`, `topCenter`,
  `topStart`, `startCenter`, `endCenter` ;
- `drawer` : tiroir de navigation de l'écran (le premier compte), glissant
  depuis le bord `side` (`start` par défaut), ouvert par l'icône de menu de
  la barre de titre ou par `ui.set(id, "open", true)`. Avec
  `permanentFrom: lg`, il reste affiché à côté du contenu à partir de cette
  classe de taille (voir « Tailles d'écran »), sans voile ni icône de
  menu ;
- `bottomSheet` : panneau montant du bas, ouvert par
  `ui.set(id, "open", true)`.

Fermé par l'utilisateur (voile, `Échap` sur desktop ; geste, toucher à
côté sur Android), un tiroir ou un panneau repasse à `open: false` (lu par
`ui.get`) et déclenche `onDismiss`.

`floating` est un conteneur (colonne) posé par-dessus les autres :

```yaml
- type: floating
  anchor: topEnd       # point de référence de x, y
  x: 16                # décalage vers l'intérieur, en pixels
  y: 16
  z: 10
  children:
    - { type: card, children: [ { type: text, value: "3 nouveaux" } ] }
```

- `position: fixed` (défaut) : par rapport à l'écran, immobile au
  défilement ; `absolute` : par rapport au conteneur parent ;
- `anchor` : `topStart` (défaut), `topCenter`, `topEnd`, `startCenter`,
  `center`, `endCenter`, `bottomStart`, `bottomCenter`, `bottomEnd` ; `x`
  et `y` décalent vers l'intérieur (depuis un centre : vers la droite et le
  bas) ;
- déplaçable depuis Ocara : `ui.set(id, "x", 120)` (et `y`, `z`).

`spinner` est un indicateur d'attente : un modèle proposé (`kind`, de
couleur `color`, principale par défaut) ou une image animée personnelle
(`src` : GIF, WEBP ou SVG animé, embarquée comme les autres images ; sur
Android, animée à partir d'Android 9). `kind` et `src` s'excluent ; `size`
(32 par défaut) fixe sa largeur et sa hauteur.

```yaml
- { type: spinner, kind: dots, size: 32, color: "#1565c0", label: Chargement }
- { type: spinner, src: images/loader.webp, size: 48 }
```

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
- **Couleurs** : `"#rrggbb"`, `"#rrggbbaa"`, `"#rgb"`, `"#rgba"`,
  `"rgb(21, 101, 192)"` ou `"rgba(21, 101, 192, 0.5)"` (composantes 0 à
  255, alpha 0 à 1), **entre guillemets** : en YAML, `#` précédé d'une
  espace commence un commentaire. Le compilateur les ramène à `#rrggbb` ou
  `#rrggbbaa`.
- **Textes** : un nombre ou un booléen donné à une propriété texte est
  converti en texte (`value: 42` → `"42"`).
- **Choix** (`options`) : `[a, b]` ou `[{ value: a, text: "Libellé" }]`,
  normalisés en `{ value, text }`.

### Listes et `bind`

`item` est le gabarit d'un élément de la liste. Dans ce gabarit, `bind:
champ` affiche le champ de chaque élément fourni par `ui.setItems`
(`bind: user.name` pour un champ imbriqué). Hors d'un gabarit `item`, `bind`
lie le nœud à un objet Ocara (voir [Liaisons à un objet](#liaisons-à-un-objet)).

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

### Liaisons à un objet

Hors d'une liste, `bind: racine.champ` lie un nœud au champ d'un objet
Ocara, dans les deux sens. La racine (`user`) est reliée à un objet par
`ui.bind("user", user)` (voir [UI.md](builtins/UI.md#liaisons)) :

```yaml
- { type: heading, bind: user.name }
- { type: input, bind: user.name }
- { type: input, kind: number, bind: user.age }
- { type: checkbox, label: "Lettre", bind: user.newsletter }
- { type: slider, min: 0, max: 10, binds: { value: user.volume } }
- { type: text, bind: user.address.city }
```

- **Code → interface** : un champ modifié par le code s'affiche après le
  gestionnaire en cours, sans `ui.set`.
- **Interface → code** : une saisie (`input`, `checkbox`/`switch`, `slider`,
  `select`/`radio`, `date`) écrit le champ, avant l'action suivante : un
  `onClick` qui suit une saisie voit l'objet à jour.
- **Propriété liée** : `bind` (sur `text`, `heading`, `input`, `image`,
  `checkbox`, `switch`) lie `value` (`checked` pour une case, `src` pour une
  image) ; `binds: { prop: racine.champ }` lie n'importe quelle autre
  propriété (`value` d'un `slider`/`select`/`date`, `visible`, `enabled`,
  `color`…). Une action ne se lie pas.
- **Chemins** : jusqu'à trois champs (`user.address.city`) ; un objet
  intermédiaire `null` affiche `null` et ignore la saisie.
- **Types** : un champ `string`, `int`, `float`, `bool` ou `mixed`
  (éventuellement `|null`) ; un nombre ou un booléen s'affiche aussi dans un
  texte. Une saisie est convertie au type du champ ; invalide (`12a` pour un
  `int`), elle n'écrit rien et le texte saisi reste affiché.
- **`id`** : un nœud lié sans `id` en reçoit un, `@` suivi du chemin
  (`@user.name`, puis `@user.name#2`…), lisible par `ui.get`.

Un `bind` hors d'une liste sans racine (`bind: name`) est une erreur.

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

### Styles d'état et effets

Dans un style ou sur un nœud, `states` associe à chaque état une **table
d'état** : des propriétés de présentation appliquées tant que le nœud est
dans cet état. Les tables sont regroupées sous `states` pour ne heurter
aucune propriété (`checked` d'une case à cocher) :

```yaml
styles:
  action:
    background: "#00796b"
    shadow: { y: 2, blur: 6, color: "rgba(0, 0, 0, 0.3)" }
    transition: { duration: 150, properties: [background, opacity] }
    states:
      hover: { background: "#004d40" }
      active: { opacity: 0.8 }
      focus: { border: "#ffb300" }
      disabled: { opacity: 0.4 }
```

| État | Quand |
|---|---|
| `hover` | le pointeur est sur le nœud (jamais sur un écran tactile) |
| `focus` | le focus est dans le nœud |
| `active` | appui en cours |
| `disabled` | `enabled: false` |
| `invalid` | champ obligatoire vide après une soumission (voir « Formulaires ») |
| `checked` | case, interrupteur ou radio cochés |
| `visited` | lien (`link`) déjà suivi ; l'état est gardé : `ui.get(id, "visited")`, `ui.set(id, "visited", false)` |

La table d'état d'un nœud complète celle de son style, et celle d'un thème
(`styles.dark.action.states.hover`) complète celle du style.

Effets :

| Propriété | Valeur | Android |
|---|---|---|
| `opacity` | 0 à 1 | |
| `blur` | flou du nœud, rayon en px | Android 12 et plus |
| `backdropBlur` | flou de ce qui est derrière le nœud (verre dépoli), rayon en px | ignoré (pas d'équivalent Compose) |
| `shadow` | `{ x, y, blur, spread, color, inset }` (valeurs absentes : 0, noir translucide), ou liste d'ombres | une ombre : couleur et flou ; décalage, `spread` et `inset` ignorés ; à travers un nœud translucide, l'ombre se voit |
| `transition` | `{ duration, delay, easing, properties }` : changements de ces propriétés (états, `ui.set`) animés ; `properties` absent : toutes | `background`, `color`, `opacity` |

`easing` : `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`.
`elevation` reste un raccourci d'ombre.

### Animations

Une animation est déclarée sous `animations:`, sur le modèle des
`@keyframes` CSS, puis posée sur un nœud :

```yaml
animations:
  pulse:
    duration: 800          # ms (défaut 300)
    delay: 0
    easing: ease-in-out
    iterations: infinite   # nombre, ou infinite (défaut 1)
    direction: alternate   # normal, reverse, alternate, alternate-reverse
    fill: none             # none, forwards, backwards, both
    keyframes:
      0%: { opacity: 1, scale: 1 }
      100%: { opacity: 0.6, scale: 1.05 }

layout:
  type: card
  animation: pulse                         # au premier affichage
  children:
    - { type: button, text: "Ok", animation: { name: pulse, on: hover }, onAnimationEnd: pulsed }
```

- **Étapes** : `0%` à `100%`, `from` (0 %) et `to` (100 %), au moins deux.
  Propriétés animables : `opacity`, `scale`, `rotate` (degrés),
  `translateX`/`translateY` (px), `background`, `color`, `blur`,
  `width`/`height` (px). Une autre propriété est une erreur.
- **Déclenchement** (`on`) : `appear` (au premier affichage, défaut),
  `hover`, `active`, `focus` (tant que l'état dure).
- **Depuis Ocara** : `ui.animate(id, nom)` relance l'animation, avec des
  options facultatives (`duration`, `delay`, `easing`, `iterations`,
  `direction`, `fill`) : `ui.animate("card", "pulse", {"iterations": 2})` ;
  `ui.stopAnimation(id)` l'arrête (voir [UI.md](builtins/UI.md#animations)).
- **Fin** : `onAnimationEnd` part à la fin d'une animation finie, avec son
  nom en `event.value()`.
- **Réglage système « réduire les animations »** : animations et
  transitions immédiates.
- **Android** : `opacity`, `scale`, `rotate`, `translateX`/`Y` et
  `background` sont animés ; `color`, `blur`, `width`/`height` ne le sont
  que sur desktop.

## Écrans inclus

`screens:` peut lister d'autres fichiers sous la clé `includes` (chemins
relatifs au fichier qui les inclut) ; chacun contient `screens:` et, s'il
le faut, les `styles:` et les `components:` de ses écrans, et peut lui-même en inclure d'autres :

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

## Composants

`components:` déclare des composants réutilisables : un `layout` (un nœud,
avec ses enfants) et des **paramètres**. Un nœud dont le `type` nomme un
composant est remplacé, à la compilation, par ce `layout` ; les moteurs de
rendu (desktop, Android) n'ont rien de plus à connaître, et tout est vérifié
comme le reste du fichier.

```yaml
components:
  carCard:
    params:
      title: text                                  # obligatoire
      price: { type: text, default: "—" }          # valeur par défaut
      photo: { type: text, required: false }       # facultatif
      onOpen: action
    layout:
      type: card
      onClick: $onOpen
      children:
        - { type: image, src: $photo, label: "Photo", height: 120 }
        - { type: text, id: title, value: $title, style: card_ref }
        - { type: text, value: "Prix : $price" }
  includes:
    - ui.d/components.oc.ui

screens:
  home:
    layout:
      type: column
      children:
        - { type: carCard, id: best, title: "Clio", price: "9 000 €", onOpen: open_best, width: fill }
        - type: list
          id: cars
          item: { type: carCard, title: { bind: model }, price: { bind: price }, onOpen: open_car }
```

- **Paramètres** : `nom: type`, ou `nom: { type, required, default }`.
  Types : `text`, `number`, `bool`, `color`, `icon`, `action`, `style`,
  `length`. La valeur de l'instance est contrôlée selon ce type ; un
  paramètre obligatoire absent est une erreur.
- **`$nom`** dans le `layout` : seul, il prend toute la valeur de l'instance
  (nombre, booléen, action…) ; dans un texte (`"Prix : $price"`), son texte.
  Un paramètre facultatif absent **retire la propriété** qui le cite
  (l'image ci-dessus n'a pas de `src` sans photo). `$nom` qui n'est pas un
  paramètre est une erreur de la déclaration.
- **`id`** : celui de l'instance va sur la racine du composant et préfixe
  ses `id` internes (`best.title`, accessible par `ui.get`/`ui.set`) ; un
  composant qui a des `id` internes se pose donc plusieurs fois avec des
  `id` différents.
- **Autres propriétés de l'instance** (`width`, `margin`, `visible`,
  `style`…) : appliquées à la racine du composant.
- **Listes** : dans un gabarit `item`, un paramètre peut lire un champ de
  l'élément (`title: { bind: model }`) ; il devient un `binds` du nœud qui
  le cite.
- **Fichiers** : `components.includes` liste des fichiers de composants ;
  un fichier inclus (par `screens.includes` ou `components.includes`) peut
  déclarer `screens:`, `styles:` et `components:`. Les composants forment un
  seul espace de noms, utilisable dans tous les écrans ; un nom déjà pris
  (type prédéfini, autre composant) est une erreur, comme un composant
  utilisé dans lui-même.
- Une erreur dans le `layout` d'un composant est située dans son fichier, à
  la ligne de la déclaration.
- **Actions propres** : une action écrite `self.nom` dans le `layout`
  appartient au composant ; le compilateur la nomme `<composant>.nom` et le
  code Ocara la lie pour toutes les instances par
  `ui.component("carCard").on("nom", …)`, `event.instance()` donnant l'`id`
  de l'instance ([UIComponent](builtins/UIComponent.md)). Un nœud qui la
  porte sans `id` en reçoit un (`clio.nom`) quand l'instance a un `id`.
  L'arbre normalisé liste les composants et leurs actions (`components`)
  et les instances qui ont un `id` (`instances`).

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

Les événements (`onClick`, `onChange`, `onSubmit`, `onSelect`, `onLoad`,
`onError`, `onMessage`, `onInvalid` et les événements communs ci-dessous),
les `webHandlers` et le `action` des entrées de menu nomment une **action**
(lettres, chiffres, `_`). Le code Ocara la lie par
`ui.on(action, Classe::méthode)`. Les actions `back` et `close` sont prises
en charge par le moteur sans `ui.on`.

## Événements

Tout nœud affiché accepte :

| Événement | Déclenché | `event.value()` |
|---|---|---|
| `onHover` / `onLeave` | le pointeur (souris, stylet) entre dans le nœud / en sort | valeur du champ, sinon `null` |
| `onFocus` / `onBlur` | le focus entre dans le nœud / en sort (pas d'un de ses enfants à l'autre) ; un nœud qui ne le prend pas de lui-même devient focalisable | valeur du champ (texte, case cochée), sinon `null` |
| `onLongPress` | appui maintenu 500 ms sans bouger (pointeur ou doigt) ; le relâchement n'est pas un clic | valeur du champ, sinon `null` |
| `onDoubleClick` | double clic ou double tape (les deux clics restent des clics) | valeur du champ, sinon `null` |
| `onSwipe` | balayage d'au moins 50 px | `"left"`, `"right"`, `"up"` ou `"down"` |

Dans un gabarit `item`, `event.value()` est l'élément (sauf pour
`onSwipe`) et `event.index()` son rang. Sur un écran tactile, `onHover`
et `onLeave` ne partent jamais.

```yaml
- type: card
  id: car
  onHover: preview_car
  onLeave: hide_preview
  onLongPress: car_menu
  onSwipe: car_swiped
  children:
    - { type: text, value: "Clio" }
```

## Formulaires

`required: true` marque un champ obligatoire (`input`, `date`, `time`,
`select`, `radio`, `checkbox`/`switch` — cochée attendue). Une action qui
**soumet** — l'`onSubmit` d'un champ, ou l'action d'un bouton
`submit: true` — n'est remise aux gestionnaires que si chaque champ
obligatoire de son formulaire est renseigné (texte non blanc, case cochée,
choix fait). Le formulaire est le conteneur `form: true` le plus proche
(`column`, `row`, `stack`, `grid`, `card`), sinon l'écran.

Sinon, rien n'est envoyé à la place de l'action :

- chaque champ vide passe à l'état `invalid` (contour rouge), levé dès qu'il
  est modifié ;
- le premier reçoit le focus ;
- l'action `onInvalid` du bouton, si elle est déclarée, part avec la liste
  des `id` des champs vides (`event.value()`).

```yaml
- type: card
  form: true
  children:
    - { type: input, id: email, kind: email, placeholder: "E-mail", required: true }
    - { type: checkbox, id: terms, label: "J'accepte", required: true }
    - { type: button, text: "Créer le compte", submit: true, onClick: create, onInvalid: missing }
```

La vérification est faite par le runtime sur l'état gardé (saisies,
`ui.set`, liaisons) : la même sur desktop et sur Android. `ui.validate(id)`
rend les champs obligatoires vides d'un conteneur, sans les marquer (voir
[UI.md](builtins/UI.md#formulaires)). Un champ `required`, un bouton `submit`
ou un champ à `onSubmit` sans `id` en reçoit un (`@input`, `@button#2`…).

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
touchées changent) et affiche les erreurs du compilateur sur les nœuds.
On y déclare et modifie aussi les composants, on pose leurs instances et on
gère les fichiers inclus (`tools/highlight/vsode/README.md`).

Dans l'éditeur texte, les couleurs ont leur pastille et leur sélecteur ;
Ctrl+clic sur un chemin d'`includes` ouvre le fichier, sur la valeur de
`start`, d'un `style` ou d'un `type` de composant mène à sa déclaration.

## Limites du YAML accepté

Un seul document par fichier ; pas d'ancres ni d'alias (`&nom`, `*nom`) ;
une clé en double dans une table est une erreur.

## Arbre normalisé

`--dump` affiche ce que le binaire embarque : `window`, `styles`, `themeStyles`, `start`,
`screens` (chaque écran : `title`, `layout`), `menu`, `icons`, `vars`,
`sizes` (seuils des classes), `variants` (arbres des variantes de taille),
`source` (build de debug : chemins pour le rechargement à chaud), plus `ids`
(chaque `id` et le type de son nœud) et `actions` (liste triée), qui
servent à vérifier les `ui.on` et les `get`/`set`, et `roots` : pour chaque
racine liée, ses chemins (`paths`) et ses nœuds (`id`, propriété, rang du
chemin, nature `text`/`number`/`bool` de la valeur), retirés des nœuds
eux-mêmes. `animations` y est normalisé : options complétées par leurs
valeurs par défaut, étapes en liste triée (`{ "at": 50, … }`) ; `shadow`
est toujours une liste d'ombres complètes, `animation` d'un nœud
`{ "name", "on" }`.

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

- [Fiche `ocara.UI`](roadmap.d/langage-builtin-ui-multiplateforme.md) : ce qui reste à faire
- [Tauri](builtins/Tauri.md), [AndroidUI](builtins/AndroidUI.md) : spécificités de chaque plateforme
- Code : `src/core/ui_schema.d/` (lecteur YAML positionné, tables du schéma, icônes, validation, aperçu)
