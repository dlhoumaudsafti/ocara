# Builtin Tauri

> ⚠️ **Statut : expérimental, en cours de développement.** L'API ci-dessous est le contrat visé, mais l'implémentation runtime n'est pas encore fonctionnelle (fenêtre, événements JS↔Ocara, dialogues et notifications sont pour l'instant des stubs). Ne pas utiliser en production.

Ce builtin permet d'interfacer Ocara avec Tauri pour créer des applications desktop multiplateformes avec une interface web native.

## Objectif
Fournir des primitives pour :
- Lancer une fenêtre Tauri
- Communiquer entre Ocara et le frontend (JS)
- Accéder aux APIs système exposées par Tauri (fichiers, notifications, etc.)

## Exemples d'utilisation
> **Remarque :** Si "width" et "height" ne sont pas définis dans l'objet passé à `use Tauri`, la fenêtre sera créée en 800×600 par défaut.
```ocara
import ocara.Tauri
import ocara.IO

var ui:Tauri = use Tauri({
    "title": "Mon App Ocara",
    "width": 800,
    "height": 600,
    "url": "index.html"
})


ui.listen("message", nameless(msg:string): void {
    IO::writeln("Message reçu du frontend : " + msg)
})

ui.emit("backendReady", {"status": true})

// Ouvrir une boîte de dialogue native
var result:string = ui.dialog({
    "type": "info",
    "title": "Information",
    "message": "Action terminée avec succès."
})
IO::writeln("Résultat du dialogue : " + result)

// Envoyer une notification système
ui.notify({
    "title": "Notification Ocara",
    "body": "Votre tâche est terminée."
})
```

## Spécification

Convention runtime : `Tauri_<méthode>`. Toutes les méthodes ci-dessous sont des méthodes d'instance, sauf le constructeur.

| Méthode Ocara | Description |
|---|---|
| `use Tauri(options:map<string,mixed>)` → `Tauri` | Constructeur — crée une fenêtre (`title`, `width`, `height`, `url`) |
| `listen(event:string, callback:Function<void(string)>)` → `void` | Enregistre un handler JS → Ocara pour un événement nommé |
| `emit(event:string, data:mixed)` → `void` | Émet un événement Ocara → JS |
| `dialog(options:map<string,mixed>)` → `string` | Ouvre une boîte de dialogue native, retourne la réponse utilisateur |
| `notify(options:map<string,mixed>)` → `void` | Envoie une notification système |
| `getTitle()` / `setTitle(title:string)` | Lit/modifie le titre de la fenêtre |
| `getWidth()` / `setWidth(width:int)` | Lit/modifie la largeur de la fenêtre |
| `getHeight()` / `setHeight(height:int)` | Lit/modifie la hauteur de la fenêtre |
| `getUrl()` / `setUrl(url:string)` | Lit/modifie l'URL affichée |
| `open()` / `close()` | Ouvre/ferme la fenêtre |
| `isOpen()` → `bool` | Vrai si la fenêtre est ouverte |
| `focus()` / `hasFocus()` → `bool` | Met au premier plan / vérifie le focus |
| `minimize()` / `maximize()` / `restore()` | Change l'état de la fenêtre |
| `isMinimized()` / `isMaximized()` → `bool` | Vérifie l'état courant |

## Exemple de communication Ocara <-> JavaScript

### Côté Ocara
```ocara
import ocara.Tauri

var ui:Tauri = use Tauri({
    "title": "Demo Tauri",
    "width": 800,
    "height": 600,
    "url": "index.html"
})


// Recevoir un message du frontend JS
ui.listen("fromJS", nameless(data:string): void {
    IO::writeln("Reçu du JS : " + data)
})

// Envoyer un message au frontend JS
ui.emit("fromOcara", {"msg": "Hello depuis Ocara !"})
```

### Côté JavaScript (frontend)
```js
// Recevoir un message du backend Ocara
window.__TAURI__.event.listen("fromOcara", (event) => {
    console.log("Reçu d'Ocara:", event.payload);
});

// Envoyer un message au backend Ocara
window.__TAURI__.event.emit("fromJS", "Hello depuis le JS !");
```

