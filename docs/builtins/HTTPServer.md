# ocara.HTTPServer

Serveur HTTP multi-connexions intégré dans le runtime Ocara. Basé sur `tiny_http`, il accepte plusieurs connexions simultanées via un pool de threads.

## Import

```ocara
import ocara.HTTPServer
```

## Création & configuration

```ocara
const server:HTTPServer = use HTTPServer()
server.set_port(8080)            // port d'écoute (défaut : 8080)
server.set_host("0.0.0.0")      // interface réseau (défaut : "0.0.0.0")
server.set_workers(4)            // threads workers (défaut : 4)
```

## Enregistrement des routes

```ocara
server.route(path:string, method:string, handler:Function)
```

- **`path`** : chemin exact, ex. `"/"`, `"/api/users"`.
- **`method`** : méthode HTTP en majuscules, ex. `"GET"`, `"POST"`.
- **`handler`** : closure ou référence de fonction `nameless(req:int): int { … }`.

```ocara
server.route("/", "GET", nameless(req:int): int {
    HTTPServer::respond(req, 200, "Hello World")
    return 0
})
```

## Démarrage

```ocara
server.run()   // bloquant — le programme attend indéfiniment
```

## Méthodes statiques — lecture de la requête

Ces méthodes sont appelées depuis l'intérieur d'un handler. Le paramètre `req`
est le handle de requête passé automatiquement au handler.

| Méthode | Signature | Description |
|---|---|---|
| `req_path` | `(req:int) → string` | Chemin de la requête (sans query string) |
| `req_method` | `(req:int) → string` | Méthode HTTP (`"GET"`, `"POST"`, …) |
| `req_body` | `(req:int) → string` | Corps de la requête |
| `req_header` | `(req:int, name:string) → string` | Valeur d'un en-tête (insensible à la casse) |
| `req_query` | `(req:int, key:string) → string` | Valeur d'un paramètre query string |

## Méthodes statiques — construction de la réponse

| Méthode | Signature | Description |
|---|---|---|
| `respond` | `(req:int, status:int, body:string) → void` | Définit le statut et le corps de la réponse |
| `set_resp_header` | `(req:int, name:string, value:string) → void` | Ajoute un en-tête à la réponse |

## Méthodes d'instance — récapitulatif

| Méthode | Signature | Description |
|---|---|---|
| `set_port` | `(port:int) → void` | Port d'écoute |
| `set_host` | `(host:string) → void` | Adresse d'écoute |
| `set_workers` | `(n:int) → void` | Nombre de threads workers |
| `route` | `(path:string, method:string, f:Function) → void` | Enregistre une route |
| `run` | `() → void` | Démarre le serveur (bloquant) |

## Exemple complet

```ocara
import ocara.HTTPServer
import ocara.IO

function main(): int {

    const server:HTTPServer = use HTTPServer()
    server.set_port(3000)
    server.set_workers(8)

    // Route GET /
    server.route("/", "GET", nameless(req:int): int {
        var name:string = HTTPServer::req_query(req, "name")
        if name == "" {
            name = "Monde"
        }
        HTTPServer::set_resp_header(req, "Content-Type", "text/plain; charset=utf-8")
        HTTPServer::respond(req, 200, `Bonjour ${name} !`)
        return 0
    })

    // Route POST /echo
    server.route("/echo", "POST", nameless(req:int): int {
        var body:string = HTTPServer::req_body(req)
        HTTPServer::set_resp_header(req, "Content-Type", "application/json")
        HTTPServer::respond(req, 200, `{"echo":"${body}"}`)
        return 0
    })

    IO::writeln("Serveur démarré sur http://localhost:3000")
    server.run()
    return 0
}
```

## Handler via méthode de classe

Il est possible de passer une méthode statique ou une fonction libre comme handler :

```ocara
import ocara.HTTPServer

class HomeController {
    public static method home(req:int): int {
        HTTPServer::respond(req, 200, "Page d'accueil")
        return 0
    }
}

function main(): int {
    const server:HTTPServer = use HTTPServer()
    server.set_port(8080)
    server.route("/", "GET", HomeController::home)
    server.run()
    return 0
}
```

> **Note** : `HomeController::home` (sans parenthèses) transmet un fat pointer vers la méthode. `HomeController::home()` appellerait la méthode immédiatement — ce n'est pas ce que l'on veut ici.

## Gestion des connexions multiples

Le modèle est **accept pool** :  
`workers` threads appellent chacun `server.recv()` en boucle. Chaque requête est traitée dans le thread qui l'a acceptée (sans handoff). Ce modèle est simple et efficace pour des charges I/O-bound.

Pour `N` connexions simultanées, utiliser `server.set_workers(N)`.

## Notes de sécurité / concurrence

- Les handlers s'exécutent en parallèle dans plusieurs threads.  
- Les closures avec captures partagées (variables `heap_promoted`) ne sont **pas** protégées par un mutex. Des accès concurrents à des données partagées constituent un data race — comportement non défini.  
- Utiliser `ocara.Thread` + mécanisme de synchronisation externe si un état partagé est nécessaire entre handlers.
