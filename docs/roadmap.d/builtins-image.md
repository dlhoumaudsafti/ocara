# Builtin de gestion d'images (`ocara.Image`)

Statut : **à concevoir puis implémenter** — ouverte le 2026-10-26, priorité
moyenne. Rien n'est implémenté ; cette fiche décrit l'existant, le besoin et
les questions à trancher.

## Besoin

Créer, charger, dessiner et enregistrer des images **hors fenêtre** (sans
`ocara.SDL` ni affichage), dans les formats courants :

- lecture et écriture : PNG, JPEG, WEBP, GIF (y compris animé), BMP ;
- à étudier : TIFF, ICO, AVIF, QOI, TGA.

Cas d'usage visés : génération de miniatures côté serveur (`HTTPServer`),
traitement par lots en ligne de commande, production d'assets pour les jeux
SDL, conversion de formats.

## Existant (vérifié)

- `ocara.SDL` sait **charger** une image en texture (`loadTexture`, via la
  feature `image` de la crate `sdl3`) et la dessiner dans une fenêtre, mais :
  il faut une fenêtre ouverte, une texture vit côté GPU, et rien ne permet
  d'enregistrer une image ni d'accéder à ses pixels.
- Aucun autre builtin ne manipule d'images.

## API envisagée (à trancher)

```ocara
import ocara.Image

var img:Image = Image::create(width: 320, height: 200)        // fond transparent
var photo:Image = Image::load("photo.jpg")                      // format déduit du contenu

img.setPixel(x: 10, y: 10, r: 255, g: 0, b: 0, a: 255)
img.fillRect(x: 0, y: 0, w: 50, h: 50, r: 0, g: 0, b: 255, a: 255)
img.drawLine(...)
img.drawImage(photo, x: 0, y: 0)                                // composition alpha
img.resize(width: 160, height: 100)                             // filtre à choisir
img.crop(...) / img.rotate(...) / img.flipH() / img.flipV()

img.save("out.png")                                             // format déduit de l'extension
img.save("out.jpg", quality: 85)
img.width() / img.height() / img.getPixel(x, y)
```

## Questions à trancher

- **Crate** : `image` (Rust pur, couvre PNG/JPEG/WEBP/GIF/BMP/TIFF/ICO/QOI…,
  WEBP en écriture limité au mode sans perte) — candidate naturelle ; pour le
  dessin vectoriel (lignes, cercles, texte), `imageproc` ou `tiny-skia`.
  Préférer du Rust pur (pas de lib C à lier) pour garder la cross-compilation
  Windows/Android simple.
- **Crate séparé** (`runtime_image`, comme `runtime_sdl`/`runtime_tauri`) pour
  ne lier les codecs que dans les programmes qui importent `ocara.Image`.
- **Pont avec `ocara.SDL`** : créer une texture depuis une `Image`
  (`win.textureFromImage(img)`) et capturer l'écran en `Image`.
- **GIF animé** : modèle de frames (liste d'`Image` + délais) ou API dédiée.
- **Erreurs** : `ImageException` (fichier illisible, format non supporté,
  dimensions invalides).
- **Mémoire** : buffers de pixels potentiellement gros — libération par le
  comptage de références, aucune copie implicite lors des passages
  d'arguments ; vérifier l'absence de fuite (`ocaraunit --leak-check`).

## Critères d'acceptation

- `docs/builtins/Image.md` et entrée dans la procédure de
  `docs/adding-builtins.md` respectée.
- Aller-retour charger → modifier → enregistrer vérifié pour chaque format
  annoncé (tests de régression avec images de référence).
- Aucune fuite sur une boucle de création/destruction d'images.
- Exemple dans `examples/builtins/`.

## Complexité

Massive — nouveau builtin complet (crate runtime, déclarations, doc, tests).
