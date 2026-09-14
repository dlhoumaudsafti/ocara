# game_sdl — mini side-scroller (ocara.SDL)

Petit jeu de plateforme 2D en défilement horizontal ("side-scroll"), écrit
entièrement en Ocara au-dessus de `ocara.SDL` (voir
[docs/builtins/SDL.md](../../../docs/builtins/SDL.md)) — pas de moteur de
jeu, juste des textures et une boucle de rendu.

## Lancer le jeu

Depuis la racine du dépôt (les textures sont chargées via des chemins
relatifs à la racine, comme `examples/builtins/sdl.oc`) :

```bash
./target/release/ocara examples/advanced/game_sdl/game.oc -o /tmp/game_sdl
/tmp/game_sdl
```

## Contrôles

| Touche | Effet |
|---|---|
| Flèche droite / gauche | Marcher |
| Maintenir Ctrl + flèche | Courir |
| Échap ou fermer la fenêtre | Quitter |

## Structure

```
game_sdl/
├── game.oc          ← point d'entrée (bloc runtime main/error) : boucle de
│                       rendu, entrées clavier, assemble Player + Level
├── Player.oc         ← le personnage : animation, sens, dessin
├── Level.oc          ← le décor : 3 plans en parallax, dessin
└── assets/
    ├── sprite.png    ← planche du personnage (marche + course), 16 frames
    └── tileset.png   ← ciel, skyline, immeubles, route
```

Deux textures en tout — chacune chargée **une seule fois** (`loadTexture`,
dans `Player.init`/`Level.init`) — pas de fichier par frame ni par tuile de
décor : chaque frame/tuile est un simple sous-rectangle de l'une des deux
planches, dessiné via `SDL.drawTextureRegion` (voir plus bas).

## `main` / `error` : bloc runtime plutôt que `function main()`

`game.oc` utilise le [bloc runtime](../../../docs/EBNF.md#5-blocs-runtime)
(`main { ... } error { ... }`) plutôt qu'une fonction `main(): int` classique :
`try`/`on SDLException` fixe `ERROR` via `result e.code` au lieu de
`return`, et le bloc `error` affiche un message si SDL est indisponible
(machine headless, pas de serveur d'affichage...).

## Rendu : 3 plans en parallax

Du plus lointain au plus proche, chacun défilant à sa propre vitesse pour
donner une impression de profondeur (le personnage reste fixe à l'écran,
c'est le décor qui défile sous ses pas — voir `Player.worldX`) :

1. **Ciel + skyline lointaine** (le plus lent)
2. **Immeubles**, au premier plan, juste derrière la route
3. **Route**, à la vitesse exacte du personnage

## `drawTextureRegion` : une seule texture, pas de découpage

`ocara.SDL` ne savait dessiner qu'une texture **entière** (`drawTexture`/
`drawTextureScaled`) — impossible donc d'utiliser directement une planche à
plusieurs sprites/tuiles sans la découper en un fichier par frame. Ce jeu a
été l'occasion d'ajouter `drawTextureRegion(textureId, srcX,srcY,srcW,srcH,
x,y,w,h, flipH)` au builtin (voir [docs/builtins/SDL.md](../../../docs/builtins/SDL.md)) :
dessine un **sous-rectangle** d'une texture, redimensionné, avec
retournement horizontal optionnel — exactement ce que fait un vrai moteur
2D avec un atlas de sprites. `Player.oc`/`Level.oc` ne chargent donc chacun
qu'**une seule fois** leur planche (`sprite.png`/`tileset.png`) et dessinent
tout le reste via des rectangles source constants, mesurés une fois sur les
planches d'origine.

`flipH` évite aussi de dupliquer une frame "regarde à gauche" : le
personnage tourné à gauche est la **même** frame que celle tournée à droite,
simplement retournée au rendu (`Player.draw`), pas un second fichier image.

**Limite assumée** : les colonnes de `sprite.png` ne sont pas parfaitement
régulières (poses de course qui débordent légèrement d'une case sur
l'autre) — les rectangles sources ont été mesurés au plus juste, mais un
pied/une main d'une pose voisine peut occasionnellement déborder de
quelques pixels sur une frame. Une planche dessinée dès le départ en cases
strictement isolées n'aurait pas ce défaut ; corriger cela demanderait de
retoucher `assets/sprite.png` lui-même, pas le code.
