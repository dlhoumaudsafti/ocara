# Builtin Tauri

> ⚠️ **Statut : partiellement fonctionnel.** La fenêtre desktop réelle (basée sur Tauri v2 / WebKitGTK) et le pont IPC JS → Ocara (`ui.handler()` / `ui.handlers()`) sont implémentés et fonctionnels. Le reste de l'API (`listen`, `emit`, `dialog`, `notify`, ainsi que les getters/setters d'état de fenêtre appelés *après* `run()`) reste une simulation en mémoire, sans effet sur la fenêtre réelle — voir la section [Ce qui est simulé](#ce-qui-est-simulé-pas-encore-branché) plus bas. Prérequis système (GTK/WebKit) : voir [README.md](../../README.md#dépendances-gui-natives-builtin-tauri).

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
- une **URL `http(s)://`** (ex. `"http://localhost:8080"`) : la fenêtre pointe alors vers un serveur déjà en cours d'exécution — typiquement un `ocara.HTTPServer` lancé sur son propre thread juste avant (voir [Exemple complet](#exemple-complet-serveur--fenêtre--ipc) et `examples/ocara_app/`).

`ui.run()` doit être appelé sur le **thread principal** (contrainte de Tauri/WebKitGTK) et bloque jusqu'à la fermeture de la fenêtre.

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

Voir `examples/ocara_app/` pour une application desktop complète : un
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
| `run()` → `void` | Ouvre la vraie fenêtre (bloquant, thread principal requis) |

### Ce qui est simulé (pas encore branché)

Ces méthodes existent et ne lèvent pas d'erreur, mais elles lisent/modifient un
état Ocara interne **déconnecté de la vraie fenêtre WebKitGTK** créée par
`run()` — les appeler avant `run()` n'a aucun effet sur la fenêtre qui
s'ouvrira, et les appeler après `run()` (depuis un handler IPC, par exemple)
ne change rien à l'affichage réel. À implémenter dans une prochaine itération.

| Méthode | Description (simulée) |
|---|---|
| `listen(event:string, callback:Function<void(string)>)` → `void` | Enregistre un callback pour un événement nommé (jamais déclenché par un vrai événement JS) |
| `emit(event:string, data:mixed)` → `void` | N'émet rien vers la vraie page — trace uniquement sur stdout |
| `dialog(options:map<string,mixed>)` → `string` | Ne montre aucune boîte de dialogue — retourne toujours `"ok"` |
| `notify(options:map<string,mixed>)` → `void` | N'envoie aucune notification système — trace uniquement sur stdout |
| `getTitle()` / `setTitle(title:string)` | Lit/modifie un titre en mémoire, pas celui de la fenêtre réelle |
| `getWidth()` / `setWidth(width:int)` | Idem pour la largeur |
| `getHeight()` / `setHeight(height:int)` | Idem pour la hauteur |
| `getUrl()` / `setUrl(url:string)` | Idem pour l'URL |
| `open()` / `close()` / `isOpen()` → `bool` | État simulé, ne ferme/rouvre pas la vraie fenêtre |
| `focus()` / `hasFocus()` → `bool` | État simulé |
| `minimize()` / `maximize()` / `restore()` | État simulé |
| `isMinimized()` / `isMaximized()` → `bool` | État simulé |
