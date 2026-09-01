# Builtin SDL

> ⚠️ **Statut : Paliers 1, 2 et 3.** Fenêtre, renderer 2D, événements
> clavier/souris/redimensionnement, dessin (Palier 1), textures/images +
> texte (Palier 2), et manettes + audio (Palier 3) sont **réellement branchés
> sur SDL3** — rien n'est simulé, contrairement au Tauri "Phase 1". Deux
> limitations structurelles à connaître avant de commencer : voir
> [Limites](#limites-du-palier-1) ci-dessous. Prérequis système (cmake +
> headers X11 dev + libpng) : voir [README.md](../../README.md).

Ce builtin permet d'ouvrir une fenêtre native et d'y dessiner en 2D avec SDL3
— fenêtrage, rendu, entrées clavier/souris, sans dépendance à un navigateur ou
une WebView (contrairement à `ocara.Tauri`).

## Import

```ocara
import ocara.SDL
```

## Créer une fenêtre et une boucle de rendu

```ocara
import ocara.SDL
import ocara.IO

var win:SDL = use SDL({ "title": "Ma fenêtre", "width": 800, "height": 600 })

var running:bool = true
while running {
    // Vider la file d'événements avant de dessiner la frame
    var ev:map<string, mixed> = win.pollEvent()
    while ev["type"] not equal "none" {
        if ev["type"] equal "quit" {
            running = false
        }
        ev = win.pollEvent()
    }

    win.setDrawColor(20, 20, 30, 255)
    win.clear()
    win.setDrawColor(220, 60, 60, 255)
    win.fillRect(100, 100, 200, 120)
    win.present()

    SDL::delay(16)   // ~60 fps
}
```

> **Remarque :** si `width`/`height`/`title` ne sont pas définis, la fenêtre
> est créée en 800×600 avec le titre `"Ocara App"`.

Contrairement à `Tauri::run()`, il n'y a **pas** de méthode `run()` bloquante :
c'est le code Ocara lui-même qui pilote la boucle (`pollEvent`/dessin/`present`
en boucle) — le modèle SDL classique.

## Lire les événements

`pollEvent()` vide la file un événement à la fois ; `{"type":"none"}` quand
elle est vide (à appeler en boucle pour tout vider avant de dessiner, voir
l'exemple ci-dessus).

| `ev["type"]` | Champs supplémentaires |
|---|---|
| `"quit"` | — (croix de fermeture **ou** quit niveau OS/session, voir note ci-dessous) |
| `"keydown"` / `"keyup"` | `key` (nom, ex. `"Escape"`, `"A"`, `"Left"`), `repeat` (`0`/`1`) |
| `"mousemotion"` | `x`, `y`, `xrel`, `yrel` |
| `"mousebuttondown"` / `"mousebuttonup"` | `button` (`1`=gauche, `2`=milieu, `3`=droit, `4`/`5`=boutons latéraux), `x`, `y` |
| `"mousewheel"` | `x`, `y` |
| `"resize"` | `width`, `height` |
| `"gamepadconnected"` / `"gamepaddisconnected"` | `gamepadId` |
| `"gamepadbuttondown"` / `"gamepadbuttonup"` | `gamepadId`, `button` (nom SDL, ex. `"south"`, `"dpup"`, `"leftshoulder"`) |
| `"gamepadaxis"` | `gamepadId`, `axis` (nom SDL, ex. `"leftx"`, `"lefttrigger"`), `value` (`-32768..32767`, gâchettes `0..32767`) |
| `"unknown"` | événement SDL hors périmètre (ignorable) |

> Brancher une manette **avant** de lancer le programme génère quand même un
> `"gamepadconnected"` dès les premiers appels à `pollEvent()` — SDL signale
> aussi les manettes déjà connectées au moment où le sous-système démarre, pas
> seulement celles branchées après coup. C'est le comportement attendu, pas un
> bug.

```ocara
var ev:map<string, mixed> = win.pollEvent()
if ev["type"] equal "keydown" {
    if ev["key"] equal "Escape" {
        running = false
    }
}
```

> **Piège fermeture de fenêtre :** cliquer sur la croix de la fenêtre n'envoie
> **pas** l'événement `Quit` de SDL (réservé à un quit niveau OS/session,
> ex. Cmd+Q sur macOS) — Ocara mappe les deux vers `"type":"quit"` pour que
> tester uniquement `ev["type"] equal "quit"` suffise dans tous les cas.

## Dessiner

```ocara
win.setDrawColor(255, 0, 0, 255)   // rouge opaque (r, g, b, a)
win.clear()                         // remplit toute la fenêtre avec la couleur courante
win.fillRect(10, 10, 100, 50)       // rectangle plein
win.drawRect(10, 10, 100, 50)       // contour de rectangle
win.drawLine(0, 0, 100, 100)
win.drawPoint(50, 50)
win.present()                       // affiche la frame dessinée (à appeler une fois par frame)
```

> Les coordonnées souris (`pollEvent`) et les primitives de dessin sont en
> `int` — SDL3 utilise en réalité des flottants en interne (précision
> sous-pixel), tronqués ici pour rester simple. Une variante `float` pourrait
> arriver dans un palier ultérieur si besoin de précision fine.

## Charger et dessiner une image (Palier 2)

```ocara
var logoId:int = win.loadTexture("logo.png")   // PNG ou JPEG
IO::writeln(`Taille native : ${win.textureWidth(logoId)}x${win.textureHeight(logoId)}`)

// Dans la boucle de rendu :
win.drawTexture(logoId, 20, 20)                    // taille native
win.drawTextureScaled(logoId, 650, 20, 120, 120)    // redimensionnée
```

`loadTexture` charge l'image **une seule fois** — le handle retourné (`int`)
est réutilisable pour tous les `drawTexture`/`drawTextureScaled` suivants,
frame après frame. La texture reste en mémoire jusqu'à la fin du programme
(pas de méthode pour la libérer en Palier 2 — voir [Limites](#limites-du-palier-1)).

## Afficher du texte (Palier 2)

```ocara
var fontId:int = win.loadFont("police.ttf", 24)   // chemin, taille en points

// Dans la boucle de rendu :
win.drawText(fontId, "Score : 42", 20, 20, 255, 255, 255, 255)   // texte blanc opaque
```

Une seule ligne de texte par appel (pas de retour à la ligne automatique),
couleur en `(r, g, b, a)` comme `setDrawColor`. Contrairement aux textures de
`loadTexture`, le rendu de texte génère et détruit une texture à chaque appel
— pas de coût mémoire cumulatif, mais éviter d'appeler `drawText` avec un
texte qui ne change pas à chaque frame si la performance est critique (charger
une fois via une texture serait plus efficace pour du texte statique).

## Manettes (Palier 3)

Connexion/déconnexion et boutons/axes arrivent via `pollEvent()` (tableau
ci-dessus) — la manette est ouverte automatiquement par Ocara dès sa
connexion, aucun appel `openGamepad()` n'est nécessaire. Pour un mouvement
continu (ex. déplacement au stick analogique), utiliser la lecture directe
plutôt que d'attendre un événement à chaque frame :

```ocara
var ev:map<string, mixed> = win.pollEvent()
if ev["type"] equal "gamepadconnected" {
    gamepadId = ev["gamepadId"]
}

// Dans la boucle de rendu, chaque frame :
if win.isButtonPressed(gamepadId, "south") {
    // saut, tir, etc.
}
var moveX:int = win.getAxis(gamepadId, "leftx")   // -32768..32767
```

Noms de boutons courants : `"south"`/`"east"`/`"west"`/`"north"` (façon
Xbox : A/B/X/Y), `"dpup"`/`"dpdown"`/`"dpleft"`/`"dpright"`,
`"leftshoulder"`/`"rightshoulder"`, `"leftstick"`/`"rightstick"`,
`"start"`/`"back"`/`"guide"`. Noms d'axes : `"leftx"`/`"lefty"`,
`"rightx"`/`"righty"`, `"lefttrigger"`/`"righttrigger"`.

> Un `gamepadId` référant à une manette jamais connectée (ou déconnectée
> depuis) est un **no-op silencieux** — `isButtonPressed` renvoie `false`,
> `getAxis` renvoie `0`. Un nom de bouton/axe non reconnu suit la même règle.

## Audio (Palier 3)

```ocara
var jumpSound:int = win.loadSound("jump.wav")   // WAV/OGG/MP3
win.playSound(jumpSound)                         // superposable à lui-même et aux autres sons

win.playMusic("theme.ogg", true)                 // en boucle, remplace la piste en cours
win.setMusicVolume(50)                           // 0-100
win.pauseMusic()
win.resumeMusic()
win.stopMusic()
```

**`playSound` est polyphonique** : appeler `playSound` plusieurs fois de
suite sur le même son les superpose (chaque appel est indépendant), utile
pour des tirs rapides ou des sons qui se chevauchent. Contrepartie : pas de
volume persistant par son dans ce palier (seul le volume global de la
musique est réglable via `setMusicVolume`).

**Un seul emplacement musique par fenêtre** : `playMusic` remplace
immédiatement la piste en cours (arrêt net, pas de fondu) — ce n'est pas une
limite de SDL3, c'est un choix pour garder l'API simple. `pauseMusic`/
`resumeMusic`/`stopMusic`/`setMusicVolume` sont des no-op silencieux si
aucune musique n'est en cours.

## Limites du Palier 1

Ce sont des contraintes de SDL lui-même, pas des choix de design Ocara :

1. **Une seule fenêtre `SDL` par processus.** SDL n'autorise qu'une seule
   pompe d'événements active à la fois. Un second `use SDL(...)` dans le même
   programme lève une `SDLException` (`code: 101`) plutôt que de planter.
   Le support multi-fenêtre est un chantier de palier ultérieur.
2. **Tous les appels sur une fenêtre doivent venir du thread qui l'a créée**
   (contrainte SDL/OS, plus stricte encore sur macOS). Appeler une méthode
   `SDL` depuis un `Thread::spawn()` lève une `SDLException` (`code: 201`)
   plutôt que de planter silencieusement ou de corrompre l'affichage.

```ocara
try {
    var a:SDL = use SDL({})
    var b:SDL = use SDL({})   // 2e fenêtre : refusé en Palier 1
} on e is SDLException {
    IO::writeln(`Erreur SDL : ${e.message} (code ${e.code})`)
}
```

## Gestion d'erreurs

Certaines opérations SDL peuvent lever une `SDLException`.

### Codes d'erreur SDLException

| Code | Nom | Opération | Description |
|------|------|-----------|-------------|
| 101 | `WINDOW_ALREADY_OPEN` | `use SDL(...)` | Une fenêtre SDL est déjà ouverte dans ce processus — une seule à la fois en Palier 1 (voir [Limites](#limites-du-palier-1)) |
| 102 | `INIT_FAILED` | `use SDL(...)` | Échec d'initialisation SDL3 : pas de serveur d'affichage disponible, échec de création de la fenêtre ou du renderer... |
| 201 | `WRONG_THREAD` | toute méthode d'instance (`pollEvent`, dessin, getters/setters...) | Appel depuis un thread différent de celui qui a créé la fenêtre (voir [Limites](#limites-du-palier-1)) |
| 301 | `TEXTURE_LOAD_FAILED` | `loadTexture(path)` | Fichier introuvable ou format d'image non supporté |
| 302 | `FONT_LOAD_FAILED` | `loadFont(path, size)` | Fichier introuvable ou police invalide |
| 401 | `AUDIO_INIT_FAILED` | `loadSound`/`playMusic` (1er appel) | Échec d'initialisation du sous-système audio (pas de périphérique audio disponible, etc.) |
| 402 | `SOUND_LOAD_FAILED` | `loadSound(path)` | Fichier introuvable ou format audio non supporté |
| 403 | `MUSIC_LOAD_FAILED` | `playMusic(path, loop)` | Fichier introuvable ou format audio non supporté |

### Exemple de gestion d'erreurs

```ocara
import ocara.SDL
import ocara.IO

function main(): int {
    try {
        var win:SDL = use SDL({ "title": "Demo", "width": 800, "height": 600 })
        // ... boucle de rendu ...
    } on e is SDLException {
        IO::writeln(`SDL error: ${e.message}`)
        IO::writeln(`Code: ${e.code}`)
        if e.code equal 101 {
            IO::writeln("Une seule fenêtre SDL par processus en Palier 1.")
        }
    }
    return 0
}
```

---

## Spécification

Convention runtime : `SDL_<méthode>`.

| Méthode | Description |
|---|---|
| `use SDL(options:map<string,mixed>)` → `SDL` | Constructeur — crée la fenêtre, le renderer et la pompe d'événements (`title`, `width`, `height`) |
| `pollEvent()` → `map<string,mixed>` | Dépile un événement (`{"type":"none"}` si la file est vide) |
| `setDrawColor(r:int, g:int, b:int, a:int)` → `void` | Fixe la couleur de dessin courante (0-255 par canal) |
| `clear()` → `void` | Remplit toute la fenêtre avec la couleur courante |
| `fillRect(x:int, y:int, w:int, h:int)` → `void` | Rectangle plein |
| `drawRect(x:int, y:int, w:int, h:int)` → `void` | Contour de rectangle |
| `drawLine(x1:int, y1:int, x2:int, y2:int)` → `void` | Ligne |
| `drawPoint(x:int, y:int)` → `void` | Point |
| `present()` → `void` | Affiche la frame dessinée depuis le dernier `present()` |
| `isOpen()` → `bool` | `false` après `close()` |
| `close()` → `void` | Marque la fenêtre comme fermée côté Ocara (voir note plus bas) |
| `getWidth()` / `getHeight()` → `int` | Taille courante (requête live, pas de cache) |
| `getTitle()` / `setTitle(title:string)` | Titre de la fenêtre |
| `SDL::ticks()` → `int` (statique) | Millisecondes écoulées depuis l'initialisation SDL |
| `SDL::delay(ms:int)` (statique) | Pause bloquante — utile pour limiter le framerate |
| `loadTexture(path:string)` → `int` | Charge une image (PNG/JPEG), retourne un handle |
| `textureWidth(textureId:int)` / `textureHeight(textureId:int)` → `int` | Dimensions natives d'une texture chargée |
| `drawTexture(textureId:int, x:int, y:int)` → `void` | Dessine une texture à sa taille native |
| `drawTextureScaled(textureId:int, x:int, y:int, w:int, h:int)` → `void` | Dessine une texture redimensionnée |
| `loadFont(path:string, size:int)` → `int` | Charge une police (.ttf/.otf) à une taille donnée, retourne un handle |
| `drawText(fontId:int, text:string, x:int, y:int, r:int, g:int, b:int, a:int)` → `void` | Rend une ligne de texte à la position et couleur données |
| `isButtonPressed(gamepadId:int, button:string)` → `bool` | État direct d'un bouton de manette |
| `getAxis(gamepadId:int, axis:string)` → `int` | État direct d'un axe de manette (`-32768..32767`) |
| `loadSound(path:string)` → `int` | Charge un son (WAV/OGG/MP3), retourne un handle |
| `playSound(soundId:int)` → `void` | Joue un son (superposable) |
| `playMusic(path:string, loop:bool)` → `void` | Charge et joue une musique en remplaçant la piste en cours |
| `pauseMusic()` / `resumeMusic()` / `stopMusic()` → `void` | Contrôle de la piste musicale en cours |
| `setMusicVolume(volume:int)` → `void` | Volume de la musique (0-100) |

> `close()` ne détruit pas la fenêtre native (elle reste ouverte à l'écran
> jusqu'à la fin du programme) — elle rend simplement `isOpen()` faux et les
> appels de dessin/événements suivants sans effet. Rouvrir une nouvelle
> fenêtre dans le même processus après `close()` n'est **pas** supporté en
> Palier 1 (voir [Limites](#limites-du-palier-1)).

> Un `textureId`/`fontId` inconnu passé à `drawTexture*`/`drawText`/
> `textureWidth`/`textureHeight` est un **no-op silencieux** (pas
> d'exception) — même philosophie permissive qu'un appel sur une fenêtre déjà
> fermée.
