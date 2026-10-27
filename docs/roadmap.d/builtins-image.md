# Builtin Image : suite (`ocara.Image`)

Statut : **base livrée**, priorité basse pour la suite. Voir
[docs/builtins/Image.md](../builtins/Image.md) pour ce qui existe.

## Existant (vérifié)

- Crate séparé `runtime_image/` (crate `image`, Rust pur), lié seulement
  dans les programmes qui importent `ocara.Image` (`src/codegen/link.rs`).
  Il n'appelle le runtime principal qu'en ABI C (`runtime_image/src/ffi.rs`) :
  aucune seconde copie du runtime dans le binaire.
- Une `Image` est une valeur native comptée (`TAG_NATIVE`,
  `runtime/src/native.rs`), rangée parmi les valeurs comptées par
  `NATIVE_COUNTED` (`src/lower/builder.d/rc_layout.rs`).
- Création, chargement, dessin (pixels, lignes, rectangles, cercles,
  composition alpha), redimensionnement, recadrage, rotation, miroirs,
  enregistrement en PNG, JPEG, WEBP (sans perte), GIF (image fixe), BMP.
- Windows : `runtime_image` est cross-compilé par `make build-runtime-windows`
  et embarqué dans `ocara.exe` ; chargement, dessin et enregistrement
  vérifiés sous Wine. Les exceptions y sont touchées par le bug général
  [windows-locales-apres-exception](windows-locales-apres-exception.md).
- Tests : `examples/tests/117_image_builtinTest.oc` (images de référence
  produites par un autre outil dans `examples/tests/images/`),
  `runtime_image/src/tests.rs`, exemple `examples/builtins/image.oc`.

## Reste à faire

- **GIF animé** : lecture de toutes les images et de leurs délais,
  écriture d'une animation. Modèle à trancher : `array<Image>` + délais, ou
  objet `Animation` dédié. `Image::load` ne lit aujourd'hui que la première
  image d'un GIF animé.
- **Pont avec `ocara.SDL`** : texture depuis une `Image`
  (`win.textureFromImage(img)`) et capture de la fenêtre en `Image`.
  `runtime_sdl` dépend d'`ocara_runtime` en Rust : passer par
  `__native_data` (ABI C) pour lire les pixels.
- **Texte sur une image** : `drawText(font, text, x, y)` (police TTF/OTF,
  crate `ab_glyph` ou `fontdue`, Rust pur).
- **Android** : `src/main.rs` refuse aujourd'hui `ocara.Image` sur une cible
  Android ; ajouter une cible `make build-runtime-image-android` et
  l'archive au lien Android (`link_android`), comme `--android-runtime-sdl`.
- **Formats supplémentaires** : TIFF, ICO, QOI, TGA (features du crate
  `image`, Rust pur) ; AVIF (décodage en C via `dav1d`, à étudier) ; WEBP
  avec perte (encodeur en C `libwebp`, à étudier).

## Complexité

Légère (formats Rust purs, texte) à Structurel (GIF animé, pont SDL, Android).
