# `ocara.UI` : styles d'état, flou, ombre portée, animations

## Demande (David)

- Styles selon l'état d'un nœud : survol (`hover`), focus, visité
  (`visited`), actif (`active`).
- Sur les éléments : un style de flou et un style d'ombre portée.
- Des animations écrites selon la convention CSS, et animables depuis
  `ocara.UI`.

## État actuel (vérifié dans le code)

- Propriétés de style (`STYLE_PROPS`, `src/core/ui_schema.d/schema.rs`) :
  `fontSize`, `bold`, `italic`, `color`, `background`, `padding`, `margin`,
  `radius`, `border`, `elevation`. Styles nommés (`styles:`) et surcharges
  par thème (`styles.dark`/`styles.light`, `themeStyles` dans l'arbre).
- Ombre : seulement `elevation`, convertie en `box-shadow` fixe sur desktop
  (`engine.js`, `decorate`) et en élévation Material sur Android ; ni
  couleur, ni décalage, ni flou réglables.
- Aucun style d'état, aucun flou, aucune opacité, aucune animation ni
  transition.

## Spécification proposée

### Styles d'état

Dans un style nommé ou sur un nœud, une table par état, mêmes propriétés
que `STYLE_PROPS` :

```yaml
styles:
  bouton:
    background: "#1565c0"
    hover: { background: "#1e88e5" }
    active: { background: "#0d47a1" }
    focus: { border: "#ffb300" }
    disabled: { color: "#9e9e9e" }
    invalid: { border: "#c62828" }
```

- États : `hover`, `focus`, `active` (appui en cours), `disabled`
  (`enabled: false`), `invalid` (champ obligatoire vide, voir
  [docs/oc-ui.md](../oc-ui.md#formulaires) ; aujourd'hui un contour rouge
  fixe, `.oc-invalid` dans engine.css et `INVALID` dans OcaraUINodes.kt),
  `checked` (case, interrupteur, radio).
- `visited` n'a de sens que pour un lien : **à trancher** — nouveau nœud
  `link` (texte cliquable avec `url` ou action) qui porte l'état `visited`
  (URL ouverte, ou action déjà déclenchée), ou abandon de `visited`.
- Desktop : règles CSS générées par style (`:hover`, `:focus-visible`,
  `:active`, `[disabled]`, `.oc-invalid`) dans une feuille propre à l'arbre.
  Android : `interactionSource` (`collectIsHoveredAsState`,
  `collectIsFocusedAsState`, `collectIsPressedAsState`).
- Combinaison avec le thème : `styles.dark.bouton.hover` surcharge
  `styles.bouton.hover`.

### Flou et ombre

| Propriété | Valeur | Desktop | Android |
|---|---|---|---|
| `blur` | rayon en px | `filter: blur()` | `Modifier.blur` (API 31+, ignoré avant) |
| `backdropBlur` | rayon en px : floute ce qui est derrière (verre dépoli) | `backdrop-filter: blur()` | pas d'équivalent Compose : ignoré, documenté (ou fond translucide de repli) |
| `shadow` | `{ x, y, blur, spread, color }` ou liste d'ombres | `box-shadow` | `Modifier.shadow(elevation, shape, ambientColor, spotColor)` (décalage approché) |
| `opacity` | 0 à 1 | `opacity` | `Modifier.alpha` |

`elevation` reste un raccourci de `shadow`.

### Animations

- **Transitions** : `transition: { duration: 200, easing: ease-out,
  properties: [background, opacity] }` sur un style ; les changements d'état
  et les `ui.set` s'animent.
- **Animations nommées**, convention CSS `@keyframes` transposée en YAML :

```yaml
animations:
  pulse:
    duration: 800
    easing: ease-in-out
    iterations: infinite
    direction: alternate
    keyframes:
      0%: { opacity: 1, scale: 1 }
      100%: { opacity: 0.6, scale: 1.05 }
```

- Propriétés animables : `opacity`, `scale`, `rotate`, `translateX`/`Y`,
  `background`, `color`, `blur`, `width`/`height` (en px) ; une autre
  propriété dans `keyframes` est une erreur du schéma.
- Sur un nœud : `animation: pulse` (au premier affichage), ou
  `animation: { name: pulse, on: hover }`.
- Depuis Ocara : `ui.animate(id, name)` (lance l'animation nommée),
  `ui.animate(id, name, options)` (durée, répétitions), `ui.stopAnimation(id)`,
  et l'action `onAnimationEnd` du nœud.
- Desktop : `@keyframes` CSS générées, Web Animations API pour
  `ui.animate`. Android : `Animatable`/`animate*AsState`,
  `infiniteRepeatable`, `keyframes { }` de Compose.
- Respect de « réduire les animations » du système
  (`prefers-reduced-motion`, réglage d'accessibilité Android) : durée 0.

## Implémentation (pistes)

1. Schéma : tables d'état dans les styles et les nœuds, `blur`,
   `backdropBlur`, `shadow`, `opacity`, `transition`, racine `animations:`,
   `animation` ; contrôle des propriétés animables ; JSON schema de
   l'éditeur et aperçu (`preview.rs`).
2. Desktop : feuille CSS générée depuis `styles`/`animations`, classes par
   nœud ; `ui.animate` par message au moteur.
3. Android : modificateurs et animations Compose.
4. `runtime_ui`/plateformes : `UI_animate`, `UI_stopAnimation`.
5. Extension VS Code (designer de composants) : édition des états et des
   animations, aperçu.

## Tests

- Schéma : état inconnu, propriété non animable, `keyframes` mal formé.
- `ci/ui_engine_test.js` : règles CSS d'état générées, animation lancée par
  message, `onAnimationEnd`.
- ocaraunit : `ui.animate` sur un `id` absent (`UIException` 102), nom
  d'animation inconnu.

## Complexité

**Structurel à Massive** — schéma, deux moteurs et l'éditeur ; le rendu
Android de `backdropBlur` et des ombres décalées n'a pas d'équivalent exact.
