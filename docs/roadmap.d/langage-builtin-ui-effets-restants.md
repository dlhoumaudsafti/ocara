# `ocara.UI` : effets et animations, ce qui reste

Les styles d'état, effets et animations sont en place (docs/oc-ui.md,
« Styles d'état et effets », « Animations ») ; restent des écarts entre les
moteurs et l'outillage.

## Android (OcaraUIEffects.kt, OcaraUINodes.kt)

- **Étapes d'animation** : `color`, `blur`, `width`/`height` ne sont animés
  que sur desktop (Compose : `animateColor` pour la couleur du texte,
  `Modifier.blur` piloté par la progression, taille par `Modifier.size`).
- **`shadow`** : seule la première ombre, sa couleur et son flou ;
  décalage `x`/`y`, `spread`, `inset` et ombres multiples ignorés (à
  dessiner dans un `drawBehind` avec un `Paint` à `setShadowLayer`, qui
  accepte décalage et couleur).
- **Ombre sous un nœud translucide** : l'ombre d'élévation est peinte sous
  toute la forme et se voit à travers une opacité partielle (desktop :
  seulement autour) — même piste `drawBehind`.
- **`backdropBlur`** : ignoré (pas d'équivalent Compose ; `RenderEffect`
  sur la couche d'arrière-plan, Android 12 et plus, à évaluer).
- **`transition`** : `background`, `color`, `opacity` seulement.

## Designer de l'extension VS Code

- Aperçu du canevas : les tables d'état (`hover`…), `shadow`, `opacity`,
  `blur` et les animations ne sont pas rendus (canvas.js a son propre
  rendu, pas effects.js).
- `animations:` (racine) ne s'édite que dans le source ; une animation
  `{ name, on }` sur un nœud s'affiche comme du texte.

## Lien

- `link` n'ouvre pas d'URL : le gestionnaire de `onClick` le fait. Une
  propriété `url` (navigateur du système, desktop et Android) est à
  étudier.
