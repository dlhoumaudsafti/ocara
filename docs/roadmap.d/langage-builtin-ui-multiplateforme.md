# Builtin `ocara.UI` : interface native Tauri et Android unifiée, décrite en `.ui` (YAML)

Statut : **analyse faite (2026-10-28), conception à trancher**, rien
d'implémenté. Objectif fixé par David : **une interface Tauri native sur
desktop et une interface Android native sur Android, unifiées par
`ocara.UI`** — un même fichier `.ui` au format YAML et un même code Ocara
donnent une application Tauri sur desktop et une application Compose sur
Android.

```yaml
window:
  title: "Mon application"
  width: 800
  height: 600

layout:
  type: column
  spacing: 12
  padding: 16
  children:
    - type: text
      id: greeting
      value: "Bonjour"
    - type: input
      id: username
      placeholder: "Votre nom"
    - type: button
      id: submit
      text: "Valider"
      onClick: submit_form
```

## 1. Ce qui existe

| Côté | En place | Fichiers |
|------|----------|----------|
| Desktop | `ocara.Tauri` : vraie fenêtre Tauri v2 (WebKitGTK), page chargée par URL ou fichier, script injecté au chargement (`initialization_script`), appels JS → méthode statique Ocara typés (`ui.handler`/`ui.handlers`). `listen`, `emit`, `dialog`, `notify` et l'état de fenêtre après `run()` sont simulés. | `runtime_tauri/src/lib.rs`, `docs/builtins/Tauri.md`, [builtins-tauri](builtins-tauri.md) |
| Android | App de `packaging/android` en **Jetpack Compose** (Material3, navigation3 déjà en dépendances) qui affiche une WebView ; pont JNI qui démarre le programme Ocara sur son propre thread et expose la JVM et la classe `OcaraBridge` ; appels Ocara → Kotlin (`runtime_android/common`) ; builtins natifs hors GUI (permissions, notifications, fichiers…). | `runtime_android_jni/`, `runtime_android/`, `packaging/android/`, [docs/android.md](../android.md) §4 et §6 |
| Langage | Choix d'implémentation selon la cible : archive liée selon `--target` et talons ailleurs (modèle `android_feature_libs`, `src/codegen/link.rs`) ; déclarations conditionnelles `when System::OS is …` (EBNF §32). | `src/codegen/link.rs`, `src/codegen/emit.d/emitter.rs` |
| Langage | `ocara.YAML` décode du YAML **à l'exécution** (runtime, `serde_yaml`) ; le compilateur ne lit pas de YAML aujourd'hui. | `runtime/src/yaml.rs` |

Les deux backends sont donc fixés :

- **Desktop : Tauri.** Fenêtres, menus, boîtes de dialogue, zone de
  notification, notifications et raccourcis passent par les API natives de
  Tauri (cœur et plugins officiels). Le contenu de la fenêtre est dessiné par
  la vue système qu'embarque toute application Tauri (WebKitGTK, WebView2,
  WKWebView) : `ocara.UI` y injecte son propre moteur de rendu, et ni le
  `.ui` ni le code Ocara n'y voient de HTML.
- **Android : Jetpack Compose**, déjà utilisé par l'app de
  `packaging/android` : composables Material3, navigation3, boîtes de
  dialogue et notifications du système.

Le choix du backend selon la cible ne demande aucun mécanisme de langage
nouveau (§8).

## 2. Principe proposé : un arbre déclaratif, un moteur de rendu par plateforme

Ne pas traduire chaque widget en appels natifs depuis le code généré, mais
faire circuler **le même arbre** jusqu'à un moteur de rendu propre à chaque
plateforme :

```
fichier .ui (YAML)
   │  compilation : lecture, validation (types, propriétés, id uniques, actions)
   ▼
arbre normalisé (JSON) embarqué dans le binaire
   │  exécution : ui.run()
   ├──► desktop : application Tauri — fenêtre, menus, dialogues, tray natifs ;
   │              contenu par le moteur injecté dans la vue de la fenêtre
   └──► Android : application Compose — moteur Kotlin (packaging/android)
                  → composables Material3, navigation3, dialogues système

événements  : moteur → { action, id, valeur } → méthode Ocara liée par ui.on(...)
mises à jour: ui.set(id, propriété, valeur) → moteur → réaffichage du seul nœud
```

Trois contrats suffisent à unifier les deux plateformes :

1. **Le schéma** : types de nœuds, propriétés, événements, unités (§4).
2. **Le magasin d'état** : chaque nœud qui a un `id` a des propriétés lisibles
   et modifiables depuis Ocara ; le moteur se met à jour à chaque modification
   (DOM d'un côté, `mutableStateMapOf` de Compose de l'autre).
3. **Le canal d'événements** : un événement porte le nom d'action déclaré dans
   le `.ui` (`onClick: submit_form`), l'`id` du nœud et sa valeur. Ocara reçoit
   tous les événements sur un même thread, l'un après l'autre (comme les
   gestionnaires d'`HTTPServer`).

Les deux moteurs restent petits : le moteur de contenu Tauri fait quelques
centaines de lignes de JS sans framework, et le moteur Compose est une
fonction récursive `Node(node)` sur le type du nœud. Le code Ocara ne voit
jamais la plateforme. Tout ce que Tauri offre en natif autour du contenu
(fenêtres, menus, dialogues, tray, notifications) est appelé par ses API, pas
imité dans la page.

## 3. Inventaire des éléments unifiables

Clé YAML proposée, puis l'équivalent de chaque côté. « ≈ » marque une
correspondance approchée.

### Fenêtres et écrans

| Concept | `.ui` | Desktop (Tauri) | Android (Compose) | Remarque |
|---------|-------|-----------------|-------------------|----------|
| Titre | `window.title` | titre de la fenêtre Tauri | `TopAppBar` (`Scaffold`) | |
| Dimensions | `window.width`, `height` | taille de la fenêtre | ignorées (plein écran) | avertissement à la compilation pour une cible Android ? |
| Taille mini, redimensionnable | `window.minWidth`, `resizable` | options de fenêtre Tauri | ignorées | propre au desktop |
| Fermeture | `ui.close()`, événement `close` | fermeture de fenêtre | `Activity.finish()` | |
| Plusieurs écrans | `screens:` (voir Navigation) | une vue affichée à la fois | une destination navigation3 | |
| Plusieurs fenêtres | `windows:` | `WebviewWindowBuilder` (Tauri) | une fenêtre ; une seconde fenêtre devient un écran | ≈ (§5) |

### Disposition

| Concept | `.ui` | Desktop | Android | Remarque |
|---------|-------|---------|---------|----------|
| Colonne | `type: column` | `display:flex; flex-direction:column` | `Column` | |
| Ligne | `type: row` | `flex-direction:row` | `Row` | |
| Superposition | `type: stack` | `position:relative` + enfants `absolute` | `Box` | |
| Grille | `type: grid`, `columns` | CSS grid | `LazyVerticalGrid` ≈ | |
| Espacement | `spacing` | `gap` | `Arrangement.spacedBy` | |
| Marges internes / externes | `padding`, `margin` | `padding`, `margin` | `Modifier.padding` (pas de marge externe : padding du parent) | |
| Alignement | `align` (`start`, `center`, `end`, `stretch`) | `align-items`, `justify-content` | `horizontalAlignment`, `verticalArrangement` | |
| Poids, remplissage | `width: fill`/`wrap`/nombre, `weight` | `flex-grow`, `width:100%` | `fillMaxWidth`, `weight` | |
| Défilement | `scroll: true` | `overflow:auto` | `verticalScroll` | |
| Séparateur | `type: divider` | `<hr>` | `HorizontalDivider` | |
| Espace libre | `type: spacer` | `flex:1` | `Spacer(weight)` | |

### Affichage

| Concept | `.ui` | Desktop | Android | Remarque |
|---------|-------|---------|---------|----------|
| Texte | `type: text`, `value`, `style` | `<span>`/`<p>` | `Text` | |
| Titre | `type: heading`, `level` | `<h1>`…`<h3>` | `Text` + `MaterialTheme.typography` | |
| Image | `type: image`, `src`, `fit` | `<img>` (`object-fit`) | `Image` (bitmap décodé) | fichiers à embarquer : §6 |
| Icône | `type: icon`, `name` | jeu d'icônes embarqué (SVG) | `Icons.*` de Material | liste de noms commune à définir |
| Progression | `type: progress`, `value` (0..1, absente : indéterminée) | `<progress>` | `LinearProgressIndicator` | |
| Badge, carte | `type: card` | `<div>` avec ombre | `Card` | |
| Liste répétée | `type: list`, `items`, `item:` (gabarit) | boucle DOM | `LazyColumn` | indispensable pour une vraie app (§4) |

### Vue web

Composant à part entière, des deux côtés : la WebView de Tauri sur desktop,
la `WebView` d'Android sur Android, placées dans la disposition comme
n'importe quel autre nœud.

| Concept | `.ui` / Ocara | Desktop (Tauri) | Android (Compose) | Remarque |
|---------|---------------|-----------------|-------------------|----------|
| Composant | `type: webview`, `id`, dimensions comme tout nœud (`width: fill`, `weight`…) | webview enfant de la fenêtre (`Window::add_child`, fonction `unstable` de Tauri v2), placée et redimensionnée sur le rectangle du nœud | `AndroidView { WebView(context) }` | une webview dédiée par composant, distincte du contenu de la fenêtre |
| Contenu | `url` (`https://…`, `http://127.0.0.1:…` d'un `HTTPServer` Ocara), `file` (fichier embarqué), `html` (texte) | `WebviewUrl::External`/`App`, chargement d'un document HTML | `loadUrl`, `loadDataWithBaseURL` | `ui.set(id, "url", …)` recharge |
| JavaScript, stockage | `javascript: true`, `storage: true` | activés par défaut dans Tauri | `settings.javaScriptEnabled`, `domStorageEnabled` | |
| Navigation | `ui.webBack(id)`, `ui.webForward(id)`, `ui.reload(id)` ; propriétés `canGoBack`, `url` (page courante) | historique de la webview | `goBack`, `goForward`, `reload`, `canGoBack` | le bouton retour Android remonte d'abord l'historique de la webview |
| Liens autorisés | `allow: ["127.0.0.1", "example.org"]` | contrôle des navigations (`on_navigation`) | `WebViewClient.shouldOverrideUrlLoading` | un lien hors liste s'ouvre dans le navigateur du système |
| Chargement | `onLoad` (valeur : URL), `onError` | `on_page_load` | `onPageFinished`, `onReceivedError` | |
| Page → Ocara | `onMessage` ; dans la page : `window.ocara.post(data)` | IPC Tauri (même pont que `ocara.Tauri`) | `addJavascriptInterface` | `event.value` : le message (texte ou JSON décodé) |
| Ocara → page | `ui.postMessage(id, data)`, `ui.eval(id, script)` → `string` | `Webview::eval` | `evaluateJavascript` | `eval` attend le résultat, comme `confirm` |
| Appel de méthode Ocara depuis la page | `webHandlers:` (`nom: action`), `window.ocara.invoke(nom, args)` → promesse | IPC Tauri typé (mécanisme de `ui.handler`) | `addJavascriptInterface` + réponse par `evaluateJavascript` | reprend l'API JS déjà documentée dans `docs/builtins/Tauri.md` |
| Zoom, agent utilisateur | `zoom`, `userAgent` | `set_zoom`, `user_agent` | `textZoom`, `userAgentString` | |

Avec ce composant, une page web (servie par un `HTTPServer` Ocara comme
dans `mini_project_hexa`) peut occuper tout l'écran ou un seul panneau, à
côté de composants natifs : `ocara.UI` couvre aussi le besoin de
[`ocara.UIHybrid`](langage-builtin-ui-hybride.md).

### Saisie

| Concept | `.ui` | Desktop | Android | Remarque |
|---------|-------|---------|---------|----------|
| Champ texte | `type: input`, `placeholder`, `value`, `kind` (`text`, `password`, `email`, `number`, `multiline`) | `<input type=…>`, `<textarea>` | `OutlinedTextField` + `KeyboardOptions` | |
| Case à cocher | `type: checkbox`, `checked`, `label` | `<input type=checkbox>` | `Checkbox` + `Text` | |
| Interrupteur | `type: switch` | case à cocher stylée | `Switch` | |
| Choix exclusif | `type: radio`, `options` | `<input type=radio>` | `RadioButton` | |
| Liste déroulante | `type: select`, `options` | `<select>` | `ExposedDropdownMenuBox` | |
| Curseur | `type: slider`, `min`, `max`, `step` | `<input type=range>` | `Slider` | |
| Date, heure | `type: date`, `time` | `<input type=date>` | `DatePicker`/`TimePicker` (boîte) | rendu différent, même valeur ISO |
| Désactivé, lecture seule | `enabled`, `readonly` | `disabled`, `readonly` | `enabled`, `readOnly` | |

### Actions et événements

| Concept | `.ui` | Desktop | Android | Remarque |
|---------|-------|---------|---------|----------|
| Bouton | `type: button`, `text`, `variant` (`primary`, `secondary`, `text`) | `<button>` | `Button`, `OutlinedButton`, `TextButton` | |
| Bouton icône | `type: iconButton` | `<button>` + icône | `IconButton` | |
| Clic | `onClick` | `click` | `onClick` | |
| Changement de valeur | `onChange` | `input`/`change` | `onValueChange` | valeur : string, bool ou nombre |
| Validation d'un champ | `onSubmit` | touche Entrée | action IME (`ImeAction.Done`) | |
| Focus | `onFocus`, `onBlur` | `focus`/`blur` | `onFocusChanged` | |
| Sélection dans une liste | `onSelect` (sur `list`) | clic sur un élément | clic sur un élément | porte l'index et l'`id` de l'élément |
| Raccourci clavier | `shortcuts:` | `keydown` | clavier physique seulement | **à moitié unifiable** (§5) |

### État

| Concept | API Ocara | Desktop | Android |
|---------|-----------|---------|---------|
| Lire une propriété | `ui.get("username", "value")` | valeur gardée par le runtime, à jour des événements | idem |
| Modifier une propriété | `ui.set("greeting", "value", "Bonjour Ada")` | message vers la page (`eval`) → DOM | message JNI → état Compose |
| Visibilité, activation | `ui.set(id, "visible", false)` | `display:none`, `disabled` | rendu conditionnel, `enabled` |
| Remplir une liste | `ui.setItems("cars", rows)` | régénération de la liste | nouvelle liste d'état |

Le runtime garde l'état de chaque nœud. Il sert `get` sans aller-retour
vers l'interface, et il permet de reconstruire l'interface après une
recréation (rotation d'écran sur Android, rechargement de page sur desktop).

### Navigation

| Concept | `.ui` / Ocara | Desktop | Android |
|---------|---------------|---------|---------|
| Écrans nommés | `screens: { home: …, details: … }`, `start: home` | une vue affichée, les autres masquées | destinations navigation3 |
| Aller à un écran | `ui.navigate("details")` | changement de vue | `backStack.add` |
| Retour | `ui.back()`, événement `back` | bouton « précédent » éventuel | bouton retour système (`BackHandler`) |

### Cycle de vie

| Concept | Événement Ocara | Desktop | Android |
|---------|-----------------|---------|---------|
| Interface prête | `ready` | page chargée, moteur JS initialisé | premier rendu Compose |
| Fermeture demandée | `close` (refusable) | `CloseRequested` de Tauri | retour sur l'écran de départ, `onDestroy` |
| Mise en arrière-plan | `pause`, `resume` | perte/reprise du focus ≈ | `onPause`/`onResume` |

### Thème, style, unités

| Concept | `.ui` | Desktop | Android | Remarque |
|---------|-------|---------|---------|----------|
| Unité de longueur | nombres en pixels logiques | px CSS (viewport `device-width`) | dp | 1 px CSS = 1 dp dans une WebView Android : la même valeur donne la même taille |
| Taille de texte | `fontSize` | px CSS | sp (suit le réglage d'accessibilité) | |
| Couleurs | `color`, `background`, `#rrggbb` | CSS | `Color` | |
| Styles nommés | `styles: { titre: { fontSize: 24, bold: true } }`, puis `style: titre` | classes CSS générées | objets `TextStyle`/`Modifier` | |
| Thème clair/sombre | `theme: auto`/`light`/`dark`, `primary` | variables CSS + `prefers-color-scheme` | `MaterialTheme` (+ couleurs dynamiques) | |
| Coins, bordure, ombre | `radius`, `border`, `elevation` | CSS | `RoundedCornerShape`, `BorderStroke`, `Card` | |

### Menus

| Concept | `.ui` | Desktop (Tauri) | Android (Compose) | Remarque |
|---------|-------|-----------------|-------------------|----------|
| Menu de l'application | `menu:` (entrées `text`, `action`, `shortcut`, sous-menus) | barre de menus native (`tauri::menu`) | actions de la `TopAppBar` + menu de débordement | même `action` que les boutons |
| Menu contextuel | `contextMenu:` sur un nœud | menu natif au clic droit (`tauri::menu`, popup) | `DropdownMenu` à l'appui long | |
| Entrée cochable, désactivée | `checked`, `enabled` | `CheckMenuItem`, `enabled` | `DropdownMenuItem` + icône de coche | |

### Services natifs

| Concept | API Ocara | Desktop (Tauri) | Android | Remarque |
|---------|-----------|-----------------|---------|----------|
| Message bref | `ui.toast(text)` | notification discrète dans la fenêtre ≈ | `Snackbar` | Tauri n'a pas de toast natif |
| Boîte d'alerte, confirmation | `ui.alert(title, text)`, `ui.confirm(…)` → `bool` | boîte native (plugin officiel `dialog` : `message`, `ask`, `confirm`) | `AlertDialog` | bloque le thread Ocara jusqu'à la réponse, comme `AndroidPermissions::request` |
| Choix de fichier, enregistrement | `ui.pickFile(filters)`, `ui.saveFile(name)` → `string` | sélecteur natif (plugin `dialog` : `open`, `save`) | Storage Access Framework | voir [builtins natifs Android : suite](packaging-android-native-features.md) |
| Notification système | `ui.notify(title, text)` | plugin officiel `notification` | `NotificationManager` (déjà `ocara.AndroidNotifications`) | permission demandée par le backend |
| Presse-papiers | `ui.copy(text)`, `ui.paste()` | plugin officiel `clipboard-manager` | `ClipboardManager` | |
| Ouvrir un lien, un fichier | `ui.open(url)` | plugin officiel `opener` | `Intent.ACTION_VIEW` | |
| Info-bulle | `tooltip` | attribut `title` | `TooltipBox` (appui long) | |
| Accessibilité | `label` (texte lu) | `aria-label` | `contentDescription` | à exiger sur `image` et `iconButton` |
| Traduction | valeurs `"@cle"` résolues par une table | idem | idem | à rapprocher du `Language` de `mini_project_hexa` |

## 4. Format `.ui` proposé

Il reprend l'exemple de David et y ajoute les écrans, les styles et les
listes :

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
            spacing: 8
            children:
              - { type: text, bind: brand }
              - { type: text, bind: model }
  details:
    title: "Détail"
    layout:
      type: column
      padding: 16
      children:
        - { type: text, id: car_title, style: titre }
        - { type: button, text: "Retour", onClick: back }
```

Règles proposées :

- Un `id` est unique dans le fichier ; seuls les nœuds qui ont un `id` sont
  accessibles depuis Ocara.
- Un événement (`onClick`, `onChange`…) nomme une **action**, liée côté
  Ocara par `ui.on`. Les actions `back` et `close` sont prédéfinies.
- Dans le gabarit `item:` d'une liste, `bind: champ` lit le champ de chaque
  élément passé à `ui.setItems`.
- Une clé `desktop:` ou `android:` sur un nœud surcharge ses propriétés pour
  cette plateforme seulement (écarts du §5).
- `type: webview` est le composant vue web du §3 ; ses messages et appels
  JS passent par les mêmes actions que les autres événements.
- Inconnu (type, propriété, valeur d'énumération) : **erreur de compilation**
  avec la ligne du `.ui`, comme une erreur de syntaxe Ocara.

## 5. Ce qui ne s'unifie pas

| Élément | Plateforme | Traitement proposé |
|---------|------------|--------------------|
| Plusieurs fenêtres | desktop | `windows:` : vraies fenêtres Tauri ; sur Android, chaque fenêtre secondaire devient un écran |
| Fenêtre sans bordure, toujours au premier plan, transparente | desktop | `desktop: { decorations: false, alwaysOnTop: true }` (options de fenêtre Tauri) |
| Icône de zone de notification (tray) et son menu | desktop | `tray:` (Tauri `TrayIconBuilder`), ignoré sur Android |
| Survol, curseur, glisser-déposer de fichiers | desktop | propriétés `desktop:` uniquement |
| Raccourcis clavier globaux | desktop | `shortcuts:` ignoré sur Android (sauf clavier physique) |
| Bouton retour système, orientation, barres système, encoche | Android | événement `back` (géré sur les deux), `android: { orientation: portrait }` |
| Gestes (balayage, appui long, tirer pour rafraîchir) | Android | `onLongPress` ≈ clic droit desktop ; le reste en `android:` |
| Composants Material propres (FAB, bottom sheet, tiroir) | Android | `android:` d'abord ; à unifier plus tard si un équivalent desktop raisonnable existe |

Le risque signalé dans la version précédente de ce ticket demeure : appauvrir
une plateforme pour l'aligner sur l'autre. La réponse proposée est un tronc
commun (§3) suffisant pour des écrans de gestion (formulaires, listes,
détails, navigation), avec les surcharges `desktop:` et `android:` pour exploiter ce que chaque
plateforme fait nativement, plutôt qu'un plus petit dénominateur commun
strict.

## 6. API Ocara proposée

<!-- doc-check: skip -->
```ocara
import ocara.UI

class GarageController {
    public static method search(ui:UI, event:UIEvent): void {
        ui.setItems("cars", CarService::find(ui.get("search", "value")))
    }

    public static method openCar(ui:UI, event:UIEvent): void {
        ui.set("car_title", "value", event.value)
        ui.navigate("details")
    }
}

function main(): void {
    const ui:UI = use UI("garage.ui")
    ui.on("search", GarageController::search)
    ui.on("open_car", GarageController::openCar)
    ui.run()     // affiche, bloque jusqu'à la fermeture
}
```

| Méthode | Rôle |
|---------|------|
| `use UI(path)` | interface décrite par le `.ui` (lu à la compilation, voir Q3) |
| `on(action, handler)` | lie une action du `.ui` à une méthode `(ui:UI, event:UIEvent): void` |
| `get(id, prop)` → `mixed` / `set(id, prop, value)` | état d'un nœud |
| `setItems(id, rows:array<map<string,mixed>>)` | contenu d'une `list` |
| `navigate(screen)`, `back()` | navigation |
| `toast`, `alert`, `confirm`, `pickFile` | services du §3 |
| `postMessage(id, data)`, `eval(id, script)`, `webBack`, `webForward`, `reload` | composant vue web du §3 |
| `run()`, `close()` | cycle de vie |

`UIEvent` : `action`, `id`, `value` (`mixed`), `index` (pour `onSelect`).

## 7. Modèle d'exécution

Les deux plateformes diffèrent ; le contrat commun doit les absorber.

| | Desktop (Tauri) | Android |
|---|---|---|
| Thread du programme Ocara | thread principal, pris par `run()` (exigence GTK) | thread lancé par le pont JNI ; le thread principal appartient à l'Activity |
| Arrivée d'un événement | thread IPC de Tauri | thread principal Android (callback Compose) |
| Mise à jour de l'interface | doit passer par la boucle de Tauri (`eval` sur la WebView) | doit passer par le thread principal (`runOnUiThread`, état Compose) |

Contrat proposé :

- Les gestionnaires d'actions s'exécutent sur **un thread Ocara dédié**,
  l'un après l'autre (jamais sur le thread d'interface : un gestionnaire
  lent ne gèle pas l'écran ; jamais en parallèle : pas de verrou à prévoir
  dans le code utilisateur).
- `ui.set`, `ui.navigate`… sont permis depuis n'importe quel thread : le
  runtime les transmet au thread d'interface.
- `run()` bloque le thread appelant jusqu'à la fermeture. Sur Android, il
  envoie l'arbre au moteur Compose et attend la fin de l'Activity.

## 8. Choix du backend selon la cible

Même schéma que les builtins `ocara.Android*`, sans mécanisme de langage
nouveau :

- un crate par backend : `runtime_ui_tauri` (réutilise `runtime_tauri` :
  fenêtre, IPC, script injecté ; ajoute le moteur JS et les mises à jour par
  `eval`) et `runtime_android/ui` (JNI vers un `OcaraUI.kt` qui porte le
  moteur Compose) ;
- `link`/`link_android` lient le crate de la cible quand `ocara.UI` est
  importé ; les signatures Cranelift sont communes (`UI_*`).

## 9. Questions à trancher

Tranché par David : Tauri natif sur desktop, Android natif (Compose) sur
Android.

1. **Lecture du `.ui`** : à la compilation (proposé : erreurs avec ligne,
   vérification des `ui.on` contre les actions déclarées, rien à livrer à
   côté de l'APK ; il faut ajouter un lecteur YAML au compilateur), ou à
   l'exécution (rechargement à chaud en mode debug) ? Les deux sont
   compatibles : compilation par défaut, rechargement en option.
2. **Liaison des actions** : `ui.on` explicite (proposé), ou liaison
   automatique à un contrôleur (`ui.controller(GarageController)` relie
   l'action `open_car` à la méthode `openCar`) ?
3. **État** : impératif (`get`/`set`, proposé pour commencer) ou liaisons
   déclaratives (`bind: user.name` sur un objet Ocara) plus tard ?
4. **Avenir d'`ocara.Tauri` et d'[`ocara.UIHybrid`](langage-builtin-ui-hybride.md)** :
   la vue web étant un composant d'`ocara.UI` (§3), `UIHybrid` n'a plus de
   raison d'être un builtin séparé. `ocara.Tauri` reste-t-il disponible
   (programmes existants), ou devient-il un alias desktop d'une fenêtre
   `ocara.UI` réduite à une vue web ?
6. **Vue web sur desktop** : webview enfant de la fenêtre (fonction
   `unstable` de Tauri v2, vraie webview isolée, proposé) ou `<iframe>` dans
   le contenu de la fenêtre (stable, mais refusé par les sites qui
   l'interdisent et sans pont JS direct pour une autre origine) ?
5. **Extension `.ui`** : YAML strict avec l'extension `.ui` (proposé, avec
   un schéma JSON pour la complétion dans VS Code), ou `.ui.yaml` pour
   bénéficier des outils YAML sans configuration ?

## 10. Découpage proposé

| Étape | Contenu | Complexité |
|-------|---------|------------|
| 1 | Schéma, lecteur YAML et validation dans le compilateur, arbre JSON embarqué, tests unitaires des erreurs | Structurel |
| 2 | Backend Tauri : fenêtre, moteur de contenu (column, row, text, input, button, checkbox), `on`/`get`/`set`, thread des gestionnaires, exemple et test ocaraunit | Structurel |
| 3 | Backend Android : `OcaraUI.kt` (moteur Compose, mêmes nœuds), événements JNI vers le thread Ocara, mises à jour vers le thread principal, vérification sur téléphone | Massive |
| 4 | Navigation (`screens`), `list`/`setItems`, styles et thème, images embarquées | Structurel |
| 5 | Composant `webview` des deux côtés : contenu, navigation, liens autorisés, messages et appels JS ↔ Ocara ; vérification avec `mini_project_hexa` dans un panneau | Structurel |
| 6 | Menus et services natifs (plugins Tauri `dialog`, `notification`, `clipboard-manager`, `opener` ; équivalents Android), plusieurs fenêtres, tray, surcharges `desktop:`/`android:` | Structurel |
| 7 | Outillage : schéma JSON du `.ui` pour l'extension VS Code, complétion des `id` et des actions dans `get`/`set`/`on`, rechargement à chaud | Structurel |

## Fichiers clés

`runtime_tauri/src/lib.rs` (fenêtre, IPC, script injecté), `runtime_android_jni/`
et `runtime_android/common/` (pont JNI et appels Kotlin), `packaging/android/`
(app Compose), `src/codegen/link.rs` (archive selon la cible), `docs/builtins/Tauri.md`,
[langage-builtin-ui-hybride](langage-builtin-ui-hybride.md),
[packaging-android-gui-native](packaging-android-gui-native.md) (rendu natif
Android : piste Compose retenue ici),
[builtins-tauri](builtins-tauri.md) (dialogues et notifications desktop).
