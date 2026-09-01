# Builtin SDL

> ⚠️ **Statut : Palier 1 (MVP).** Fenêtre, renderer 2D, polling d'événements
> (fermeture/clavier/souris/redimensionnement) et primitives de dessin de base
> sont **réellement branchés sur SDL3** — rien n'est simulé, contrairement au
> Tauri "Phase 1". Pas encore disponible : textures/images (SDL_image), fonts
> (SDL_ttf), audio, manettes — paliers suivants. Deux limitations structurelles
> à connaître avant de commencer : voir [Limites](#limites-du-palier-1)
> ci-dessous. Prérequis système (cmake + headers X11 dev) : voir
> [README.md](../../README.md).

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
| `"unknown"` | événement SDL hors périmètre Palier 1 (ignorable) |

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

> `close()` ne détruit pas la fenêtre native (elle reste ouverte à l'écran
> jusqu'à la fin du programme) — elle rend simplement `isOpen()` faux et les
> appels de dessin/événements suivants sans effet. Rouvrir une nouvelle
> fenêtre dans le même processus après `close()` n'est **pas** supporté en
> Palier 1 (voir [Limites](#limites-du-palier-1)).
