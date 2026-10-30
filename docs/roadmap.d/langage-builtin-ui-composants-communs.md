# `ocara.UI` : panneau latéral et bouton flottant sur desktop, spinner, conteneur flottant

## Demande (David)

- Le panneau latéral de menu (`android.drawer`) et le bouton flottant
  (`android.fab`) existent sur Android : on doit pouvoir faire la même chose
  avec Tauri.
- Un spinner animé comme élément : une image animée personnelle, sinon un
  choix parmi plusieurs spinners proposés.
- Un conteneur flottant, fixe ou non, positionnable en x, y, z.

## État actuel (vérifié dans le code)

- `android.fab`, `android.bottomSheet`, `android.drawer`
  (`src/core/ui_schema.d/schema.rs`) : rendus par Compose ; sur desktop,
  `engine.js` pose un `<span hidden>` (« Composants propres à Android :
  rien sur desktop »).
- `progress` : barre de progression déterminée (`value` de 0 à 1)
  seulement ; aucun indicateur d'attente indéterminé.
- Mise en page : `column`, `row`, `stack`, `grid`, `card` ; aucun
  positionnement absolu ni fixe, aucun ordre d'empilement (`z`).

## Spécification proposée

### Panneau latéral, feuille du bas, bouton flottant communs

- Nouveaux types sans préfixe : `drawer`, `bottomSheet`, `fab`. Les noms
  `android.*` restent acceptés (alias, avertissement du schéma suggérant le
  nom commun) — **à trancher** : les retirer après une version.
- `drawer` sur desktop : panneau glissant depuis le bord (`side: start|end`),
  voile sur le contenu, fermeture par le voile ou `Échap` ; ouvert par
  `ui.set(id, "open", true)` comme sur Android (`open`, `onDismiss`).
  Option `modal: false` : panneau permanent à partir d'une classe de taille
  (`permanentFrom: lg`), comme le « navigation drawer » permanent de
  Material.
- `bottomSheet` sur desktop : panneau montant du bas, mêmes règles.
- `fab` sur desktop : bouton rond flottant en bas à droite (`position:
  bottomEnd|bottomStart|topEnd|topStart`), au-dessus du contenu, `text`
  facultatif (bouton étendu).

### Spinner

```yaml
- { type: spinner, kind: dots, size: 32, color: "#1565c0" }
- { type: spinner, src: images/loader.webp, size: 48 }
```

- `kind` : `circle` (défaut, indicateur Material), `dots`, `bars`, `pulse`,
  `ring`, `orbit` — jeu commun aux deux moteurs (CSS + SVG sur desktop,
  Canvas/animations Compose sur Android).
- `src` : image animée personnelle (GIF, WEBP animé, SVG animé), embarquée
  comme les autres images (`images.rs`) ; Android : décodeur
  `AnimatedImageDrawable` (API 28+) ou Coil-GIF à évaluer.
- `label` (accessibilité), `visible` pour l'afficher pendant un
  traitement ; `progress` sans `value` pourrait devenir une barre
  indéterminée en complément.

### Conteneur flottant

```yaml
- type: floating
  position: fixed        # fixed : par rapport à la fenêtre ; absolute : au parent
  x: 16
  y: 16
  z: 10
  anchor: topEnd         # coin de référence de x, y
  children:
    - { type: card, children: [ … ] }
```

- `position: fixed` (reste en place au défilement) ou `absolute` (relatif
  au conteneur parent, qui devient un repère) ; `x`, `y` en px ou `%`,
  `anchor` (`topStart`, `topEnd`, `bottomStart`, `bottomEnd`, `center`),
  `z` (ordre d'empilement ; un `z` sur tout nœud d'un `stack` aussi).
- Déplaçable depuis Ocara : `ui.set(id, "x", 120)` (et `y`, `z`), animable
  (voir [langage-builtin-ui-styles-animations.md](langage-builtin-ui-styles-animations.md)).
- Desktop : `position: fixed|absolute`, `z-index`. Android : `Box` racine
  avec `Modifier.offset`/`zIndex` pour `fixed`, `Box` du parent pour
  `absolute`.

## Implémentation (pistes)

1. Schéma : types `drawer`, `bottomSheet`, `fab`, `spinner`, `floating`,
   alias `android.*`, propriétés `x`, `y`, `z`, `anchor`, `position` ;
   images animées dans `images.rs`.
2. Desktop : rendus dans `engine.js`/`engine.css` (panneaux, voile,
   spinners, positionnement).
3. Android : renommage des nœuds existants, spinners, `floating`.
4. Documentation : `docs/oc-ui.md` (section « Composants propres à
   Android » réécrite en composants communs), `docs/builtins/UI.md`.
5. Éditeur de l'extension VS Code : palette et aperçu des nouveaux nœuds.

## Tests

- Schéma : alias `android.*`, `spinner` avec `kind` et `src` ensemble
  (erreur), `x`/`y` hors d'un `floating` ou d'un `stack`.
- `ci/ui_engine_test.js` : ouverture/fermeture du `drawer` (voile, Échap),
  `fab` positionné, spinner de chaque `kind`, `floating` fixe au défilement.
- Exemple : `examples/ui/garage.oc` avec un `drawer` de menu et un `fab`
  sur les deux plateformes.

## Complexité

**Structurel** — surtout du rendu dans les deux moteurs ; les alias
`android.*` évitent de casser les `.oc.ui` existants.
