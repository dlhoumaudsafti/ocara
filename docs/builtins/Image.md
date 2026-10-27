# Builtin Image

Images en mémoire, **hors fenêtre** : créer, charger, dessiner, transformer
et enregistrer des images sans `ocara.SDL` ni affichage — miniatures côté
serveur, traitement par lots, production d'assets, conversion de formats.

| Format | Lecture | Écriture |
|--------|---------|----------|
| PNG    | oui     | oui      |
| JPEG   | oui     | oui (qualité réglable, sans transparence) |
| WEBP   | oui     | oui, sans perte uniquement |
| GIF    | oui (première image d'un GIF animé) | oui (image fixe) |
| BMP    | oui     | oui      |

Le format est reconnu **au contenu** du fichier à la lecture, **à
l'extension** à l'écriture (`.png`, `.jpg`/`.jpeg`, `.webp`, `.gif`, `.bmp`).

Codecs en Rust pur (crate `image`) : aucune bibliothèque système à
installer. Ils ne sont liés que dans les programmes qui importent
`ocara.Image`.

## Import

```ocara
import ocara.Image
```

## Exemple

```ocara
import ocara.Image

function main(): void {
    var photo:Image = Image::load("photo.jpg")
    photo.resize(320, 200)

    var logo:Image = Image::load("logo.png")
    photo.drawImage(logo, 10, 10)          // composition alpha

    photo.setDrawColor(255, 255, 255, 160) // blanc semi-transparent
    photo.fillRect(0, 170, 320, 30)

    photo.saveJpeg("miniature.jpg", 85)
    photo.save("miniature.webp")
}
```

## Modèle

- Une `Image` est une grille de pixels RGBA 8 bits (rouge, vert, bleu,
  alpha de 0 à 255). Coordonnées en pixels, origine en haut à gauche,
  `y` vers le bas.
- Une `Image` est une **référence** : l'affecter ou la passer en argument ne
  copie pas ses pixels (`img.copy()` le fait). Elle est libérée avec sa
  dernière référence, comme une chaîne ou un tableau.
- Les transformations (`resize`, `crop`, `rotate`, `flipH`, `flipV`)
  modifient l'image **sur place**.
- Les primitives de dessin utilisent la **couleur courante**
  (`setDrawColor`, noir opaque par défaut), la composent par-dessus l'image
  selon son alpha, et ignorent ce qui dépasse de l'image. `setPixel` et
  `clear` remplacent les pixels tels quels, alpha compris.
- Une même image peut être utilisée depuis plusieurs threads : chaque appel
  est exécuté sous un verrou propre à l'image.

## Référence des méthodes

### `Image::create(width, height)` → `Image`

Image de `width` × `height` pixels, entièrement transparente.
`ImageException` 105 si une dimension est nulle ou négative, ou si l'image
dépasse 2²⁸ pixels.

```ocara
var img:Image = Image::create(width: 640, height: 480)
```

### `Image::load(path)` → `Image`

Charge un fichier PNG, JPEG, WEBP, GIF ou BMP (format reconnu au contenu).
`ImageException` 101 si le fichier est illisible, 102 si son contenu n'est
pas une image reconnue ou est corrompu.

### `img.width()` → `int`

Largeur en pixels.

### `img.height()` → `int`

Hauteur en pixels.

### `img.copy()` → `Image`

Nouvelle image indépendante : mêmes pixels, même couleur de dessin.

### `img.save(path)` → `void`

Enregistre l'image au format donné par l'extension de `path` (JPEG en
qualité 90). `ImageException` 104 si l'extension n'est pas un format
d'écriture pris en charge, 103 si le fichier ne peut pas être écrit.

```ocara
img.save("sortie.png")
img.save("sortie.webp")
```

### `img.saveJpeg(path, quality)` → `void`

Enregistre en JPEG avec une qualité de 1 à 100 (`ImageException` 107 sinon).
Le JPEG n'a pas de transparence : l'alpha est ignoré.

### `img.getPixel(x, y)` → `array<int>`

Composantes `[r, g, b, a]` du pixel. `ImageException` 106 si `(x, y)` est
hors de l'image.

```ocara
var p:array<int> = img.getPixel(10, 20)
IO::writeln(`rouge=${p[0]} alpha=${p[3]}`)
```

### `img.setPixel(x, y, r, g, b, a)` → `void`

Remplace le pixel par cette couleur (sans composition). `ImageException` 106
si `(x, y)` est hors de l'image, 107 si une composante sort de 0..255.

### `img.setDrawColor(r, g, b, a)` → `void`

Couleur des primitives de dessin et de `clear`. `ImageException` 107 si une
composante sort de 0..255.

### `img.clear()` → `void`

Remplit toute l'image avec la couleur courante, sans composition.

### `img.drawPoint(x, y)` → `void`

Un pixel.

### `img.drawLine(x1, y1, x2, y2)` → `void`

Segment d'un pixel d'épaisseur, extrémités comprises.

### `img.drawRect(x, y, w, h)` → `void`

Contour d'un rectangle, un pixel d'épaisseur. `ImageException` 105 si `w` ou
`h` est négatif ; rien n'est dessiné si l'un vaut 0.

### `img.fillRect(x, y, w, h)` → `void`

Rectangle plein. Mêmes règles que `drawRect`.

### `img.drawCircle(cx, cy, radius)` → `void`

Cercle d'un pixel d'épaisseur centré en `(cx, cy)`. `ImageException` 105 si
`radius` est négatif.

### `img.fillCircle(cx, cy, radius)` → `void`

Disque plein (un seul pixel pour un rayon 0).

### `img.drawImage(src, x, y)` → `void`

Compose `src` par-dessus l'image, son coin supérieur gauche en `(x, y)` : les
pixels transparents de `src` laissent voir l'image, les pixels
semi-transparents se mélangent. `src` peut être l'image elle-même.

### `img.resize(width, height)` → `void`

Redimensionne l'image (filtre de Lanczos). Mêmes dimensions valides que
`Image::create`. Pour garder les proportions, calculer l'une des dimensions
à partir de l'autre.

### `img.crop(x, y, w, h)` → `void`

Ne garde que le rectangle donné, qui doit être entièrement dans l'image
(`ImageException` 106 sinon, 105 si `w` ou `h` n'est pas positif).

### `img.rotate(degrees)` → `void`

Tourne l'image de `degrees` dans le sens horaire (négatif : sens
anti-horaire). Un multiple de 90° est exact et échange largeur et hauteur
pour 90° et 270°. Un autre angle agrandit l'image pour tout contenir, les
coins ajoutés étant transparents, avec un échantillonnage bilinéaire.
`ImageException` 107 si l'angle n'est pas un nombre fini.

```ocara
img.rotate(90.0)
img.rotate(-12.5)
```

### `img.flipH()` → `void`

Miroir horizontal (gauche ↔ droite).

### `img.flipV()` → `void`

Miroir vertical (haut ↔ bas).

## Gestion d'erreurs

Toutes les erreurs lèvent une `ImageException` (source `"Image"`), qui
hérite d'`Exception`.

### Codes d'erreur ImageException

| Code | Cause |
|------|-------|
| 100 | La valeur n'est pas une `Image` |
| 101 | Fichier illisible (absent, droits) |
| 102 | Contenu non reconnu comme image, ou image corrompue |
| 103 | Fichier impossible à écrire |
| 104 | Extension sans format d'écriture pris en charge |
| 105 | Dimensions invalides (nulles, négatives, image de plus de 2²⁸ pixels) |
| 106 | Pixel ou rectangle hors de l'image |
| 107 | Valeur invalide : composante de couleur hors de 0..255, qualité JPEG hors de 1..100, angle non fini |

```ocara
try {
    var img:Image = Image::load(chemin)
} on e is ImageException {
    IO::writeln(`Image illisible (${e.code}) : ${e.message}`)
}
```

## Plateformes

Linux et Windows. Pas encore disponible sur Android : la compilation d'un
programme Android qui importe `ocara.Image` échoue avec un message explicite.

## Voir aussi

- [SDL](SDL.md) — fenêtre, rendu à l'écran et textures
- Exemple : [examples/builtins/image.oc](../../examples/builtins/image.oc)
