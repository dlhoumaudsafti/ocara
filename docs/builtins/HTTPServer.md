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
server.set_host("0.0.0.0")       // interface réseau (défaut : "0.0.0.0")
server.set_workers(32)           // threads workers (défaut : 4)
server.set_root_path("./public") // répertoire pour fichiers statiques (optionnel)
```

> **Note** : Toutes les méthodes `set_*` sont optionnelles. Les valeurs par défaut sont adaptées pour un petit site web.

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
| `set_root_path` | `(path:string) → void` | Répertoire racine pour fichiers statiques |
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

## Fichiers statiques

HTTPServer peut servir des fichiers statiques (HTML, CSS, JS, images, etc.) depuis un répertoire racine défini avec `set_root_path()`.

### Fonctionnement

1. **Routes dynamiques en priorité** : si une route correspond au chemin demandé, le handler est appelé
2. **Fallback fichiers statiques** : si aucune route ne correspond et qu'un `root_path` est défini, le serveur cherche un fichier correspondant
3. **404 si aucun match** : ni route ni fichier → erreur 404

### Exemple

```ocara
import ocara.HTTPServer
import ocara.IO

function main(): int {
    const server:HTTPServer = use HTTPServer()
    server.set_port(8080)
    server.set_root_path("./public")

    // Route dynamique API
    server.route("/api/hello", "GET", nameless(req:int): int {
        HTTPServer::set_resp_header(req, "Content-Type", "application/json")
        HTTPServer::respond(req, 200, `{"message":"Hello API"}`)
        return 0
    })

    IO::writeln("Serveur sur http://localhost:8080")
    IO::writeln("  /api/hello → route dynamique")
    IO::writeln("  /index.html → ./public/index.html")
    IO::writeln("  /css/style.css → ./public/css/style.css")
    server.run()
    return 0
}
```

Structure du répertoire `public/` :
```
public/
  index.html
  css/
    style.css
  js/
    app.js
  images/
    logo.png
```

Requêtes :
- `GET /api/hello` → route dynamique (JSON)
- `GET /index.html` → `./public/index.html` (HTML)
- `GET /css/style.css` → `./public/css/style.css` (CSS)
- `GET /images/logo.png` → `./public/images/logo.png` (PNG)
- `GET /unknown.txt` → 404 (fichier inexistant)

### MIME types automatiques

Le serveur détecte automatiquement le `Content-Type` selon l'extension :

| Extension | Content-Type |
|-----------|--------------|
| `.html`, `.htm` | `text/html; charset=utf-8` |
| `.css` | `text/css; charset=utf-8` |
| `.js` | `application/javascript; charset=utf-8` |
| `.json` | `application/json; charset=utf-8` |
| `.txt` | `text/plain; charset=utf-8` |
| `.png` | `image/png` |
| `.jpg`, `.jpeg` | `image/jpeg` |
| `.svg` | `image/svg+xml` |
| `.woff`, `.woff2` | `font/woff`, `font/woff2` |
| autres | `application/octet-stream` |

### Sécurité

- **Protection path traversal** : les chemins contenant `..` sont rejetés automatiquement
- **Lecture seule** : seules les requêtes `GET` tentent de servir des fichiers statiques
- **Canonicalisation** : le chemin final doit rester dans le répertoire `root_path`

Exemple de requêtes bloquées :
```
GET /../etc/passwd    → bloqué (path traversal)
GET /../../secret.txt → bloqué (path traversal)
POST /index.html      → ignoré (routes dynamiques prioritaires)
```

## Gestion des connexions multiples

Le modèle est **accept pool** :  
`workers` threads appellent chacun `server.recv()` en boucle. Chaque requête est traitée dans le thread qui l'a acceptée (sans handoff). Ce modèle est simple et efficace pour des charges I/O-bound.

### Configuration des workers

La méthode `set_workers(n)` définit le nombre de threads de traitement parallèle.

> **Valeur par défaut** : `4` workers (si `set_workers()` n'est pas appelé)

**Important** : Les workers définissent la **capacité de traitement parallèle**, pas le nombre maximum de connexions simultanées. Des milliers de clients peuvent se connecter, mais seuls `N` requêtes seront traitées en parallèle à un instant donné.

### Tableau de dimensionnement

| Utilisateurs simultanés | Workers recommandés | Type de charge |
|-------------------------|---------------------|----------------|
| 10-100 | 4-8 | Défaut, petit site |
| 100-500 | 16 | Site moyen |
| 500-2000 | 32 | Site populaire |
| 2000-5000 | 64 | Haute charge |
| 5000+ | 128+ | Très haute charge (considérer un load balancer) |

**Facteurs à considérer** :
- **Temps de traitement par requête** : plus les requêtes sont longues (DB, APIs externes), plus il faut de workers
- **Nombre de CPU cores** : éviter de dépasser `cores × 2-4` pour du calcul intensif
- **Type de charge** : I/O-bound (fichiers, réseau) supporte plus de workers que CPU-bound (calculs)

**Exemple** :
```ocara
// Site web avec 1000 utilisateurs et APIs + base de données
const server:HTTPServer = use HTTPServer()
server.set_workers(32)  // Bon compromis pour cette charge
server.run()
```

## Notes de sécurité / concurrence

- Les handlers s'exécutent en parallèle dans plusieurs threads.  
- Les closures avec captures partagées (variables `heap_promoted`) ne sont **pas** protégées par un mutex. Des accès concurrents à des données partagées constituent un data race — comportement non défini.  
- Utiliser `ocara.Thread` + mécanisme de synchronisation externe si un état partagé est nécessaire entre handlers.
