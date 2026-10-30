# `ocara.UI` : `backdropBlur` partout sur Android

Les effets et animations sont alignés entre les moteurs (docs/oc-ui.md,
« Styles d'état et effets », « Animations ») ; reste le flou d'arrière-plan
sur Android.

## État actuel (vérifié dans le code)

- `OcaraUIShadows.kt` : le contenu de l'écran est enregistré dans un
  `GraphicsLayer` (`backdropSource`), et un nœud de la couche de l'écran
  (`LocalBackdrop` fourni autour du `fab` et des `floating` fixes) dessine
  ce calque flouté (`BlurEffect`, Android 12 et plus) sous sa forme.
- Ailleurs, `backdropBlur` est ignoré :
  - **nœud dans le flux de l'écran, `floating` en `position: absolute`** :
    le nœud fait partie du calque qu'il devrait dessiner (cycle de
    `RenderNode`) ;
  - **`drawer`, `bottomSheet`** : rendus par Material (`ModalDrawerSheet`,
    `ModalBottomSheet`, ce dernier dans une autre fenêtre), sans accès au
    calque.

## Pistes

- Flux de l'écran : enregistrer, pour chaque nœud flouté, un calque de ce
  qui est dessiné avant lui (frères précédents et ancêtres, sans lui ni la
  suite), comme la bibliothèque Haze (`hazeSource`/`hazeEffect`) ; ou
  dépendre de Haze (dépendance Gradle à ajouter au paquet Android).
- Tiroir : peindre le fond de `ModalDrawerSheet` avec le calque de l'écran
  (même fenêtre) ; panneau inférieur : capture par `PixelCopy` de la fenêtre
  principale à l'ouverture, floutée (image figée, à évaluer).

## Tests

- Démo Android : `floating` à `backdropBlur` au-dessus d'une liste qui
  défile, nœud flouté dans le flux, tiroir flouté ; captures comparées à
  l'aperçu desktop.

## Complexité

**Structurel** — rendu Compose seulement ; schéma, desktop et designer sont
déjà faits.
