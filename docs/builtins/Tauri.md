# Builtin Tauri

> **Statut :** la fenêtre desktop réelle (basée sur Tauri v2 / WebKitGTK), son état (titre, taille, URL, réduction, agrandissement, focus, fermeture), le pont IPC JS → Ocara (`ui.handler()` / `ui.handlers()`), les événements entre la page et Ocara (`emit` / `listen`), les boîtes de dialogue, les notifications et les menus natifs sont fonctionnels. Plusieurs fenêtres, icône de zone de notification et raccourcis globaux sont à venir ([roadmap](../roadmap.d/builtins-tauri.md)). Prérequis système (GTK/WebKit) : voir [README.md](../../README.md#dépendances-gui-natives-builtin-tauri).

Ce builtin permet d'interfacer Ocara avec Tauri pour créer des applications desktop multiplateformes avec une interface web native (WebKitGTK sur Linux).

## Import

```ocara
import ocara.Tauri
```

## Créer et ouvrir une fenêtre

```ocara
import ocara.Tauri

var ui:Tauri = use Tauri({
    "title":  "Mon App Ocara",
    "width":  800,
    "height": 600,
    "url":    "index.html"
})

ui.run()   // bloquant — ouvre la vraie fenêtre, rend la main à la fermeture
```

> **Remarque :** si `width`/`height` ne sont pas définis, la fenêtre est créée en 800×600.

`url` peut être :
- un **chemin de fichier local** (ex. `"index.html"`) : résolu depuis le répertoire courant du binaire compilé au moment de l'exécution ;
- une **URL `http(s)://`** (ex. `"http://localhost:8080"`) : la fenêtre pointe alors vers un serveur déjà en cours d'exécution — typiquement un `ocara.HTTPServer` lancé sur son propre thread juste avant (voir [Exemple complet](#exemple-complet-serveur--fenêtre--ipc) et `examples/advanced/tauri_httpserver/`).

`ui.run()` doit être appelé sur le **thread principal** (contrainte de Tauri/WebKitGTK). Il bloque jusqu'à la fermeture de la fenêtre, puis rend la main : le code qui le suit s'exécute.

## État de la fenêtre

Avant `run()`, les méthodes d'état règlent la fenêtre qui s'ouvrira. Pendant `run()`, elles lisent et modifient **la vraie fenêtre**, depuis un handler IPC comme depuis un autre thread Ocara :

```ocara
import ocara.Tauri
import ocara.Thread
import ocara.IO

function main(): void {
    var ui:Tauri = use Tauri({ "title": "Éditeur", "width": 800, "height": 600, "url": "index.html" })

    const watcher:Thread = use Thread()
    watcher.run(nameless(): void {
        Thread::sleep(1000)
        ui.setTitle("Éditeur — document.txt")
        ui.setWidth(1024)
        IO::writeln(`${ui.getWidth()}×${ui.getHeight()}`)
    })
    watcher.detach()

    ui.run()
    IO::writeln("fenêtre fermée")
}
```

- `setUrl` accepte une URL complète ou un chemin relatif à la page courante (`"second.html"`).
- `getWidth`/`getHeight` et `setWidth`/`setHeight` sont en pixels logiques, comme les options de `use Tauri`.
- `open()` affiche la fenêtre au premier plan, même réduite ; `close()` la ferme et `run()` rend alors la main.
- Après `run()`, `isOpen()` vaut `false`.

Exemple complet, qui se ferme seul : `examples/advanced/tauri_window/`.

## Événements entre la page et Ocara

`ui.emit(event, data)` envoie `data` (encodé en JSON) aux écouteurs de la page ; `ui.listen(event, callback)` reçoit ce que la page envoie, sous forme de **texte JSON** (`JSON::decode` le lit) :

```ocara
import ocara.Tauri
import ocara.JSON
import ocara.IO

function main(): void {
    var ui:Tauri = use Tauri({ "title": "Import", "url": "index.html" })

    ui.listen("start", nameless(data:string): void {
        const request:map<string, mixed> = JSON::decode(data)
        IO::writeln(`import demandé : ${request["file"]}`)
        ui.emit("progress", { "step": 1, "total": 3 })
    })

    ui.run()
}
```

Côté page, le script injecté par Ocara expose :

```js
// Page → Ocara (ui.listen) : data est encodé en JSON
window.ocara.emit("start", { file: "voitures.csv" });

// Ocara → page (ui.emit) : data arrive décodé ; rend une fonction de désabonnement
const stop = window.ocara.on("progress", (data) => console.log(data.step, data.total));
```

- Les callbacks `listen` s'exécutent sur la boucle de Tauri, comme les handlers : `ui.emit`, `ui.setTitle`… y sont permis.
- Une exception levée dans un callback `listen` est signalée sur la sortie d'erreur (`Tauri : écouteur de '…' : exception non rattrapée (…)`) ; la fenêtre continue.
- `emit` est sans effet avant `run()` (aucune page chargée) ; plusieurs `listen` sur le même événement s'exécutent tous, dans l'ordre d'enregistrement.

Exemple complet, qui se ferme seul : `examples/advanced/tauri_events/`.

## Boîtes de dialogue et notifications

Boîtes natives du système (plugin officiel `dialog` de Tauri) et notifications système (plugin `notification`), **pendant `run()`** : depuis un handler, un écouteur `listen` ou un autre thread. Chaque boîte bloque l'appelant jusqu'à la réponse de l'utilisateur, la fenêtre restant réactive.

```ocara
import ocara.Tauri
import ocara.IO

function main(): void {
    var ui:Tauri = use Tauri({ "title": "Notes", "url": "index.html" })

    ui.listen("export", nameless(data:string): void {
        const path:string = ui.saveFile("notes.md")
        if path equal "" {
            return
        }
        if ui.confirm("Exporter", `Écrire les notes dans ${path} ?`) {
            IO::writeln(`export vers ${path}`)
            ui.notify("Notes", "Export terminé")
        }
    })

    ui.run()
}
```

- `pickFile(["png", "jpg"])` filtre par extension ; `pickFile([])` accepte tous les fichiers. `pickFile` et `saveFile` rendent `""` si l'utilisateur annule.
- Avant `run()`, ces méthodes lèvent `TauriException` 102 (aucune fenêtre ouverte) ; une notification que le système refuse lève 103.
- `dialog(options)`, forme d'origine, reste disponible : `type` `"info"`, `"warning"`, `"error"` (rend `"ok"`), `"confirm"` (rend `"ok"` ou `"cancel"`), `"open"`, `"save"` (rend le chemin), avec `title` et `message`.

## Menus

`setMenu(items)` pose la barre de menus de la fenêtre ; `setContextMenu(items)` remplace le menu du clic droit de la page (`setContextMenu([])` le supprime). Chaque entrée est une map :

| Clé | Rôle |
|-----|------|
| `text` | Libellé |
| `action` | Action déclenchée par le clic : les callbacks `ui.listen(action, …)` sont appelés |
| `shortcut` | Raccourci clavier (`"CmdOrCtrl+O"`, `"Shift+F5"`…), barre de menus uniquement |
| `enabled` | `false` grise l'entrée (défaut `true`) |
| `checked` | Entrée cochable, état initial ; le callback reçoit `{"checked": true}` ou `false` |
| `separator` | `true` : séparateur |
| `items` | Sous-menu (les entrées de premier niveau de la barre en sont) |

```ocara
import ocara.Tauri

function main(): void {
    var ui:Tauri = use Tauri({ "title": "Éditeur", "url": "index.html" })

    ui.setMenu([
        { "text": "Fichier", "items": [
            { "text": "Ouvrir…", "action": "open", "shortcut": "CmdOrCtrl+O" },
            { "separator": true },
            { "text": "Quitter", "action": "quit" }
        ] },
        { "text": "Affichage", "items": [
            { "text": "Mode sombre", "action": "dark", "checked": false }
        ] }
    ])
    ui.setContextMenu([{ "text": "Copier", "action": "copy" }])

    ui.listen("open", nameless(data:string): void {
        ui.setTitle(ui.pickFile(["txt"]))
    })
    ui.listen("quit", nameless(data:string): void {
        ui.close()
    })

    ui.run()
}
```

`setMenu`/`setContextMenu` s'utilisent avant `run()` comme pendant (le menu est alors reconstruit). L'inspecteur web de WebKit est désactivé par défaut : `setDevTools(true)` (avant `run()`) l'autorise, `openDevTools()` l'ouvre.

Exemple complet : `examples/advanced/tauri_menus/`.

## Appeler du code Ocara depuis le JS de la page (IPC)

`ui.handler()` (un enregistrement) et `ui.handlers()` (plusieurs à la fois) exposent une méthode statique Ocara comme commande que le JS de la page peut appeler directement — un vrai appel entrant dans du code Ocara compilé, pas une requête HTTP.

```ocara
import ocara.Tauri
import ocara.DateTime

class Backend {
    public static method greet(name:string): string {
        return "Bonjour " + name + " ! Il est " + DateTime::fromTimestamp(DateTime::now()) + "."
    }
    public static method ping(): string {
        return "pong"
    }
}

var ui:Tauri = use Tauri({ "title": "Demo IPC", "url": "index.html" })

// Un seul handler :
ui.handler("greet", Backend::greet)

// Plusieurs à la fois (même registre anti-doublon que ui.handler) :
ui.handlers({
    "ping":  Backend::ping,
    "greet": Backend::greet
})

ui.run()
```

Côté JS (`index.html`), deux conventions d'appel sont possibles :

```js
// Forme objet nommé — clés = noms des paramètres Ocara. C'est la forme native
// du pont IPC de Tauri, toujours disponible :
const msg = await window.__TAURI_INTERNALS__.invoke("greet", { name: "Ada" });

// Forme tableau positionnel — plus proche de ce qu'on écrirait pour une
// fonction ordinaire. Elle passe par window.ocara.invoke (voir plus bas) :
const msg2 = await window.ocara.invoke("greet", ["Ada"]);
const pong = await window.ocara.invoke("ping", []);
```

**Pourquoi deux formes, et pourquoi `window.ocara.invoke` plutôt que le
`window.__TAURI_INTERNALS__.invoke` natif pour la forme tableau ?** Le pont IPC
natif de Tauri traite tout tableau JS passé en payload top-level comme du
binaire brut (pas du JSON) — un array ne lui arrive donc jamais correctement.
Ocara contourne ça en injectant, au chargement de chaque page, un petit script
qui expose `window.ocara.invoke(cmd, payload, options)` : identique à l'appel
natif, mais si `payload` est un tableau, il est réécrit en objet nommé (grâce
aux noms de paramètres enregistrés par `ui.handler`/`ui.handlers`) avant
d'être transmis au vrai pont IPC. Utilisez la forme qui vous convient — les
deux finissent par appeler la même méthode Ocara, avec la même vérification
de types.

### Typage strict et erreurs

Le compilateur connaît la signature réelle de la méthode ciblée et génère un
point d'entrée qui :
1. vérifie que chaque paramètre attendu est présent avec le bon type JSON
   (`int`, `float`, `bool` ou `string` — tableaux/maps pas encore supportés
   comme paramètres) ;
2. si tout est valide, appelle réellement la méthode Ocara et encode son
   retour ;
3. sinon, rejette la promesse JS avec un message d'erreur précis, **sans
   jamais appeler la méthode**.

```js
try {
    await window.ocara.invoke("greet", [42]);   // "name" doit être string, pas int
} catch (e) {
    console.error(e);   // "Backend_greet attend {name:string} (argument manquant ou mal typé)"
}
```

### Doublons de nom

`ui.handler`/`ui.handlers` partagent un seul registre par fenêtre : enregistrer
deux fois le même nom de commande (via `ui.handler` et/ou `ui.handlers`, dans
n'importe quel ordre) lève une `TauriException` (`code: 101`), interceptable
avec `try`/`on` :

```ocara
ui.handler("ping", Backend::ping)
try {
    ui.handlers({ "ping": Backend::ping })   // doublon
} on e is TauriException {
    IO::writeln(`Erreur : ${e.message} (code ${e.code})`)
}
```

## Exemple complet (serveur + fenêtre + IPC)

Voir `examples/advanced/tauri_httpserver/` pour une application desktop complète : un
`ocara.HTTPServer` sert un mini-site sur son propre thread, et la fenêtre Tauri
affiche directement son URL — la page d'accueil contient un bouton qui appelle
une méthode Ocara réelle via `ui.handlers()`.

```ocara
import ocara.IO
import ocara.Thread
import ocara.Tauri
import configs.Server

const server:Server = use Server()
server.routes()

const serverThread:Thread = use Thread()
serverThread.run(nameless(): void {
    server.start()
})
Thread::sleep(200)   // laisser le serveur démarrer avant de charger l'URL

var ui:Tauri = use Tauri({
    "title": "Ocara App", "width": 1000, "height": 700,
    "url": "http://localhost:8080"
})
ui.handlers({
    "greet":      Backend::greet,
    "serverTime": Backend::serverTime
})
ui.run()
```

## Spécification

Convention runtime : `Tauri_<méthode>`.

### Fonctionnel

| Méthode | Description |
|---|---|
| `use Tauri(options:map<string,mixed>)` → `Tauri` | Constructeur — crée une fenêtre (`title`, `width`, `height`, `url`) |
| `handler(name:string, method:mixed)` → `void` | Enregistre `method` (référence de méthode statique, ex. `Classe::methode`) comme commande IPC nommée `name` |
| `handlers(map:mixed)` → `void` | Enregistre plusieurs commandes en une fois (`{"nom": Classe::methode, ...}`) — même registre anti-doublon que `handler` |
| `run()` → `void` | Ouvre la vraie fenêtre (bloquant jusqu'à sa fermeture, thread principal requis) |
| `listen(event:string, callback:Function<void(string)>)` → `void` | Appelle `callback` avec le texte JSON de chaque `window.ocara.emit(event, data)` de la page |
| `emit(event:string, data:mixed)` → `void` | Envoie `data` (encodé en JSON) aux écouteurs `window.ocara.on(event, …)` de la page |
| `alert(title:string, text:string)` → `void` | Message d'information |
| `confirm(title:string, text:string)` → `bool` | Confirmation : vrai si l'utilisateur valide |
| `pickFile(extensions:array<string>)` → `string` | Choix d'un fichier (`""` si annulé) |
| `saveFile(name:string)` → `string` | Choix du chemin d'enregistrement, nom proposé `name` (`""` si annulé) |
| `notify(title:string, text:string)` → `void` | Notification système |
| `dialog(options:map<string,mixed>)` → `string` | Forme d'origine des boîtes (voir plus haut) |
| `setMenu(items:array<map<string,mixed>>)` → `void` | Barre de menus de la fenêtre (voir [Menus](#menus)) |
| `setContextMenu(items:array<map<string,mixed>>)` → `void` | Menu du clic droit de la page ; `[]` le supprime |
| `setDevTools(enabled:bool)` → `void` | Autorise l'inspecteur web (avant `run()`) |
| `openDevTools()` → `void` | Ouvre l'inspecteur web |
| `getTitle()` / `setTitle(title:string)` | Titre de la fenêtre |
| `getWidth()` / `setWidth(width:int)` | Largeur intérieure (pixels logiques) |
| `getHeight()` / `setHeight(height:int)` | Hauteur intérieure (pixels logiques) |
| `getUrl()` / `setUrl(url:string)` | Page affichée ; `setUrl` navigue (URL complète ou chemin relatif à la page courante) |
| `open()` / `close()` / `isOpen()` → `bool` | Affiche au premier plan (même réduite) / ferme (`run()` rend la main) / fenêtre visible |
| `focus()` / `hasFocus()` → `bool` | Donne le focus / a le focus |
| `minimize()` / `maximize()` / `restore()` | Réduit / agrandit / restaure |
| `isMinimized()` / `isMaximized()` → `bool` | Réduite / agrandie |
