# ocara.HTTPServer / ocara.HTTPServerRequest / ocara.HTTPServerSession

Serveur HTTP multi-connexions intégré dans le runtime Ocara. Basé sur `tiny_http`, il accepte plusieurs connexions simultanées via un pool de threads.

`HTTPServer` gère la configuration et le cycle de vie du serveur. Chaque requête reçue par un handler est représentée par un objet **`HTTPServerRequest`** — une classe dédiée (pas un entier opaque comme avant, voir la note historique en bas de page) qui donne accès au chemin, aux en-têtes, au corps, aux paramètres (query string ET corps `urlencoded`/`multipart`), et permet de construire la réponse.

## Import

```ocara
import ocara.HTTPServer
import ocara.HTTPServerRequest
import ocara.HTTPServerSession   // sessions / état global (optionnel)
```

`HTTPServerRequest` doit être importé séparément dès qu'un handler l'utilise comme type de paramètre (`nameless(req:HTTPServerRequest): int { ... }`) ou appelle une de ses méthodes. De même pour `HTTPServerSession` (voir « Sessions et état global »).

## Création & configuration

```ocara
const server:HTTPServer = use HTTPServer()
server.port(8080)            // port d'écoute (défaut : 8080)
server.host("0.0.0.0")       // interface réseau (défaut : "0.0.0.0")
server.workers(32)           // threads workers (défaut : 4)
server.rootPath("./public") // répertoire pour fichiers statiques (optionnel)
```

> **Note** : Toutes ces méthodes de configuration sont optionnelles. Les valeurs par défaut sont adaptées pour un petit site web.

## Enregistrement des routes

```ocara
server.route(path:string, method:string, handler:Function)
```

- **`path`** : chemin exact (`"/"`, `"/api/users"`) ou un chemin avec des **paramètres de segment** (voir ci-dessous).
- **`method`** : méthode HTTP en majuscules, ex. `"GET"`, `"POST"`.
- **`handler`** : closure ou référence de fonction `nameless(req:HTTPServerRequest): int { … }`.

```ocara
server.route("/", "GET", nameless(req:HTTPServerRequest): int {
    req.respond(200, "Hello World")
    return 0
})
```

### Paramètres de chemin — `<nom:type>`

Un segment de chemin entre `<` et `>` capture une portion de l'URL, la valide selon le type déclaré, et l'expose côté handler via `req.param(nom)`/`req.params()` (voir « Paramètres unifiés » plus bas — un paramètre de chemin est fusionné dans le bucket `"GET"`, exactement comme un paramètre de query string).

```ocara
server.route("/voitures/<id:int>", "GET", nameless(req:HTTPServerRequest): int {
    var id:int = req.param("id")   // déjà un int — parsé/validé par le routeur
    req.respond(200, `Voiture #${id}`)
    return 0
})
```

Types supportés : `int`, `float`, `bool` (`"true"`/`"false"` uniquement), `string` (accepte n'importe quel texte, décodé URL). Un chemin peut contenir plusieurs paramètres, éventuellement mêlés à des segments littéraux :

```ocara
server.route("/voitures/<car_id:int>/entretiens", "POST", nameless(req:HTTPServerRequest): int {
    var carId:int = req.param("car_id")
    // ...
    return 0
})
```

**Si un segment ne correspond pas au type déclaré, la route entière ne matche PAS** — pas de valeur `0`/vide substituée, pas d'erreur non gérée : la requête retombe simplement sur la route suivante (ou sur une 404 si aucune route ne matche). Concrètement, `GET /voitures/abc` contre une route `/voitures/<id:int>` ne déclenche jamais ce handler ; si une AUTRE route littérale existe pour le même chemin exact (ex. `/voitures/ajouter`), c'est elle qui matche — aucune règle de priorité "route statique avant route dynamique" à connaître : un segment littéral ne matche simplement jamais un paramètre typé qui échouerait à le parser.

Le nombre de segments doit correspondre EXACTEMENT : `/voitures/<id:int>` ne matche pas `/voitures/1/extra`.

> **Compilé une seule fois** — chaque pattern de route (`<...>` compris) est analysé à l'enregistrement (`server.route(...)`), jamais reparsé à chaque requête entrante.

## Pages d'erreur personnalisées

Vous pouvez définir des handlers personnalisés pour les codes d'erreur HTTP (404, 500, etc.) :

```ocara
server.routeError(code:int, handler:Function)
```

- **`code`** : code d'erreur HTTP (404, 500, 403, etc.)
- **`handler`** : closure appelée quand ce code d'erreur est déclenché

```ocara
server.routeError(404, nameless(req:HTTPServerRequest): int {
    var path:string = req.path()
    var html:string = `<!DOCTYPE html>
<html>
    <head><title>404 - Page non trouvée</title></head>
    <body>
        <h1>Erreur 404</h1>
        <p>La page ${path} n'existe pas.</p>
    </body>
</html>`
    req.respond(404, html)
    return 0
})
```

Si aucun handler d'erreur n'est défini, le serveur retourne une page d'erreur par défaut.

## Fonctionnalités automatiques

### Content-Type par défaut

Toutes les réponses reçoivent automatiquement l'en-tête :
```
Content-Type: text/html; charset=utf-8
```

Vous pouvez le remplacer avec `respondHeader` :

```ocara
// Réponse HTML (Content-Type automatique)
server.route("/page", "GET", nameless(req:HTTPServerRequest): int {
    req.respond(200, "<h1>Hello</h1>")
    return 0
})

// Réponse JSON (Content-Type personnalisé)
server.route("/api", "GET", nameless(req:HTTPServerRequest): int {
    req.respondHeader("Content-Type", "application/json")
    req.respond(200, `{"status":"ok"}`)
    return 0
})
```

### Index automatique

Si vous définissez un `root_path` et qu'aucune route ne correspond à `GET /`, le serveur cherche automatiquement `/index.html` :

```ocara
server.rootPath("./public")

// GET /           → cherche ./public/index.html (automatique)
// GET /index.html → cherche ./public/index.html (explicite)
// GET /about.html → cherche ./public/about.html
```

Cela permet de servir votre page d'accueil sans définir de route pour `/`.

## Démarrage

```ocara
server.run()   // bloquant — le programme attend indéfiniment
```

## ocara.HTTPServerRequest — lecture de la requête

Ces méthodes s'appellent en sucre d'instance sur l'objet `req` reçu par un handler (`nameless(req:HTTPServerRequest): int { ... }`, ou un paramètre de même type sur une méthode de contrôleur enregistrée comme handler).

| Méthode | Signature | Description |
|---|---|---|
| `path` | `() → string` | Chemin de la requête (sans query string) |
| `method` | `() → string` | Méthode HTTP réelle de la requête (`"GET"`, `"POST"`, …) |
| `body` | `() → string` | Corps brut de la requête |
| `header` | `(name:string) → string` | Valeur d'un en-tête — recherche **insensible à la casse** ; chaîne vide si absent |
| `headers` | `() → map<string, string\|int\|float\|bool\|null>` | Tous les en-têtes, clés dans leur **casse d'origine** (voir note ci-dessous) |
| `query` | `(key:string) → string` | Valeur d'un paramètre de la query string (historique — voir `param`/`params` ci-dessous pour l'accès unifié incluant le corps) |
| `param` | `(key:string, method:string\|null = null) → mixed` | Accesseur universel — voir « Paramètres unifiés » |
| `params` | `() → map<string, map<string, mixed>>` | Tous les paramètres, regroupés par méthode — voir « Paramètres unifiés » |
| `cookie` | `(name:string) → string` | Valeur d'un cookie du header `Cookie` ; chaîne vide si absent |
| `session` | `() → HTTPServerSession` | Session du visiteur, créée au besoin — voir « Sessions et état global » |

> **Note sur `headers()`** : le type de retour déclaré (`string|int|float|bool|null`) est une union par parité de forme avec `params()` — en pratique, un en-tête HTTP est **toujours** une chaîne sur le fil, `headers()` ne retourne donc jamais autre chose qu'une `string`. Contrairement à `header(name)` (recherche insensible à la casse), les **clés** de la map retournée par `headers()` conservent la casse exacte envoyée par le client.

```ocara
server.route("/echo", "GET", nameless(req:HTTPServerRequest): int {
    IO::writeln(`Path    : ${req.path()}`)
    IO::writeln(`Method  : ${req.method()}`)
    IO::writeln(`UA      : ${req.header("user-agent")}`)   // insensible à la casse
    req.respond(200, req.body())
    return 0
})
```

## ocara.HTTPServerRequest — construction de la réponse

| Méthode | Signature | Description |
|---|---|---|
| `respond` | `(status:int, body:string) → void` | Définit le statut et le corps de la réponse |
| `respondHeader` | `(name:string, value:string) → void` | Ajoute un en-tête à la réponse |

## Paramètres unifiés — `param()` / `params()`

Au-delà de la query string (`query`/historique), `HTTPServerRequest` parse automatiquement le **corps** de la requête selon son `Content-Type` :

- **`application/x-www-form-urlencoded`** : un formulaire HTML classique (`<form method="POST">` sans `enctype`).
- **`multipart/form-data`** : un formulaire avec upload de fichier (`<form method="POST" enctype="multipart/form-data">`).
- **`application/json`** : **hors périmètre de `param`/`params`** — décodez-le vous-même avec `JSON::decode(req.body())`.

### `params(): map<string, map<string, mixed>>`

Retourne TOUJOURS exactement ces 10 clés (méthodes HTTP), chacune une `map<string, mixed>` (vide si non applicable) :

```
CONNECT, DELETE, GET, HEAD, OPTIONS, PATCH, POST, PUT, QUERY, TRACE
```

- **`params()["GET"]`** : les paramètres de la query string de l'URL — **toujours peuplé**, quelle que soit la méthode réelle de la requête (une query string peut accompagner un `POST`, un `DELETE`, etc.) — **plus les paramètres de CHEMIN** (`<nom:type>`, voir « Paramètres de chemin » plus haut), également fusionnés ici.
- **`params()[<méthode réelle>]`** : les paramètres du corps, peuplés **uniquement si** la méthode réelle de la requête est celle-ci, ET que le corps n'est pas vide, ET que `Content-Type` est reconnu (`urlencoded` ou `multipart`). Si la méthode réelle est `GET` et qu'il y a *aussi* un corps (rare), le corps est fusionné dans le **même** bucket `"GET"` — en cas de collision de clé avec la query string, le corps l'emporte.
- Tous les autres buckets restent des maps vides.

**Précédence en cas de collision de clé, DEUX règles "plus spécifique l'emporte"** (même bucket `"GET"`, deux sources différentes) :
1. Un **paramètre de chemin** l'emporte sur une **query string** de même clé (`/voitures/<id:int>` appelée avec `?id=999` : `param("id")` retourne l'`id` du CHEMIN, jamais celui de la query string — le chemin est plus spécifique/intentionnel qu'une query string arbitraire).
2. Le **corps** (POST/PUT/...) l'emporte sur la **query string** de même clé (voir la règle de `param()` ci-dessous) — un paramètre de chemin n'est en revanche jamais en concurrence avec le corps (chemin et corps vivent dans des buckets différents sauf si la méthode réelle est `GET`, cas où seule la règle 1 s'applique).

### `param(key:string, method:string|null = null): mixed`

Accesseur pour une seule valeur, avec une règle de précédence pratique :

- **`method` fourni** (insensible à la casse, ex. `"post"`/`"POST"`) : cherche `key` **uniquement** dans le bucket de cette méthode (même structure que `params()`).
- **`method` omis (`null`, valeur par défaut)** : cherche d'abord dans `"GET"` (query string ET paramètres de chemin, le chemin l'emportant déjà à ce stade — voir ci-dessus), puis dans le bucket de la méthode **réelle** de la requête — le corps l'emporte en cas de collision de clé. Si la méthode réelle est `GET`, il n'y a qu'un seul bucket à consulter.
- **Absent** : retourne la représentation `mixed` "rien" habituelle (comme une clé manquante dans n'importe quelle `map<string, mixed>` ailleurs dans le langage).
- Pour un champ **fichier** uploadé via `multipart/form-data` (voir ci-dessous), la valeur retournée est elle-même une `map<string, mixed>` — narrowez-la explicitement : `var file:map<string,mixed> = req.param("avatar")`.
- Pour un paramètre de **chemin** (`<id:int>`), la valeur retournée est déjà correctement typée (un `int` réel, pas une chaîne à convertir) — `var id:int = req.param("id")` fonctionne directement, sans `Convert::strToInt`.

```ocara
server.route("/search", "GET", nameless(req:HTTPServerRequest): int {
    var q:mixed = req.param("q")   // query string
    req.respond(200, `Recherche : ${q}`)
    return 0
})

server.route("/cars", "POST", nameless(req:HTTPServerRequest): int {
    // Formulaire HTML classique (enctype par défaut = urlencoded)
    var brand:mixed = req.param("brand")
    var model:mixed = req.param("model")
    req.respondHeader("Location", "/cars")
    req.respond(302, "")
    return 0
})
```

### Upload de fichiers (`multipart/form-data`)

Un champ `<input type="file">` (ou tout part multipart avec `filename`) devient une `map<string, mixed>` avec ces 4 clés :

| Clé | Type | Description |
|---|---|---|
| `filename` | `string` | Nom de fichier envoyé par le client |
| `contentType` | `string` | `Content-Type` déclaré par le part ; `"application/octet-stream"` si absent |
| `size` | `int` | Taille du contenu en octets |
| `content` | `array<int>` | Contenu **brut**, un octet par élément (0-255) |

**Pourquoi `array<int>` et pas `string` pour `content`** : une chaîne Ocara n'est fiable que si son contenu est de l'UTF-8 valide (`ptr_to_str` retombe silencieusement sur une chaîne vide sinon) — un fichier binaire réel (image, PDF...) ne l'est presque jamais. `array<int>` est la convention **déjà établie** par ce langage pour du contenu binaire (voir [`File::readBytes`/`writeBytes`](File.md)), réutilisée ici plutôt que d'inventer une troisième convention.

```ocara
server.route("/upload", "POST", nameless(req:HTTPServerRequest): int {
    var title:mixed = req.param("title")               // champ texte simple
    var file:map<string,mixed> = req.param("avatar")    // champ fichier

    var filename:string    = file["filename"]
    var contentType:string = file["contentType"]
    var size:int            = file["size"]
    var content:array<int> = file["content"]

    File::writeBytes("./uploads/" + filename, content)
    req.respond(200, `Fichier ${filename} (${contentType}, ${size} octets) reçu`)
    return 0
})
```

> **Limitation connue** : plusieurs parts multipart portant le **même** nom de champ (ex. plusieurs fichiers soumis sous `photos[]`) — seul le dernier est conservé, aucune erreur n'est levée. Hors périmètre pour l'instant.

## ocara.HTTPServerSession — sessions et état global

### Session du visiteur

`req.session()` retourne la session du visiteur courant. Elle est identifiée par le cookie **`OCARASESSID`** (128 bits aléatoires, `Path=/; HttpOnly; SameSite=Lax`), posé automatiquement dans la réponse à la première utilisation. Les requêtes suivantes qui renvoient ce cookie retrouvent les mêmes données.

| Méthode | Signature | Description |
|---|---|---|
| `id` | `() → string` | Identifiant de la session (32 caractères hexadécimaux) |
| `set` | `(key:string, value:mixed) → void` | Enregistre une valeur pour ce visiteur |
| `get` | `(key:string) → mixed` | Valeur enregistrée ; `null` si la clé est absente |
| `has` | `(key:string) → bool` | Vrai si la clé a été posée, même avec la valeur `null` |
| `remove` | `(key:string) → void` | Retire une clé |
| `destroy` | `() → void` | Supprime la session et expire le cookie (déconnexion) |

```ocara
server.route("/login", "POST", nameless(req:HTTPServerRequest): int {
    var sess:HTTPServerSession = req.session()
    sess.set("user", req.param("name"))
    req.respond(200, "Bienvenue")
    return 0
})

server.route("/me", "GET", nameless(req:HTTPServerRequest): int {
    var sess:HTTPServerSession = req.session()
    if not sess.has("user") {
        req.respond(401, "Non connecté")
        return 0
    }
    var user:string = sess.get("user")
    req.respond(200, `Bonjour ${user}`)
    return 0
})

server.route("/logout", "GET", nameless(req:HTTPServerRequest): int {
    req.session().destroy()
    req.respond(200, "Au revoir")
    return 0
})
```

Un identifiant envoyé par le client mais inconnu du serveur (session détruite, serveur redémarré, valeur forgée) n'est jamais adopté. Une nouvelle session est créée à la place, ce qui protège contre la fixation de session.

### État global

Méthodes **statiques**. Elles gèrent un magasin clé/valeur unique, partagé par toutes les requêtes et tous les visiteurs (cache applicatif, compteur…) :

| Méthode | Signature | Description |
|---|---|---|
| `setGlobal` | `(key:string, value:mixed) → void` | Enregistre une valeur globale |
| `getGlobal` | `(key:string) → mixed` | Valeur globale ; `null` si absente |
| `hasGlobal` | `(key:string) → bool` | Vrai si la clé a été posée |
| `removeGlobal` | `(key:string) → void` | Retire une clé globale |

```ocara
server.route("/hits", "GET", nameless(req:HTTPServerRequest): int {
    var hits:int = 0
    if HTTPServerSession::hasGlobal("hits") {
        hits = HTTPServerSession::getGlobal("hits")
    }
    HTTPServerSession::setGlobal("hits", hits + 1)
    req.respond(200, `Visites : ${hits + 1}`)
    return 0
})
```

### Valeurs stockées

- **Copie profonde** : `set`/`setGlobal` copient la valeur (scalaires, `string`, `array`, `map`, imbriqués), et chaque `get`/`getGlobal` en renvoie une copie neuve. Modifier la variable d'origine après `set` ne change donc pas la valeur stockée, et la valeur survit à la fin du handler.
- Un conteneur concret (`array<int>`, `map<string, float>`…) est restitué avec la même représentation : `var cart:array<int> = sess.get("cart")`.
- **Objets et fonctions refusés** : `HTTPServerException`, code `102`.
- **Concurrence** : sessions et état global sont protégés par un verrou interne. Ils sont donc sûrs aussi depuis un `ocara.Thread` en dehors des handlers.
- **Durée de vie** : en mémoire, par processus. Aucune persistance entre redémarrages et **aucune expiration automatique** : une session vit jusqu'à `destroy()` ou l'arrêt du serveur.
- Le handle `HTTPServerSession` n'est valide que pendant l'exécution du handler, comme `req`. Ne le conservez pas au-delà.

## Méthodes d'instance — récapitulatif (HTTPServer)

| Méthode | Signature | Description |
|---|---|---|
| `port` | `(port:int) → void` | Port d'écoute |
| `host` | `(host:string) → void` | Adresse d'écoute |
| `workers` | `(n:int) → void` | Nombre de threads workers |
| `rootPath` | `(path:string) → void` | Répertoire racine pour fichiers statiques |
| `route` | `(path:string, method:string, f:Function) → void` | Enregistre une route |
| `routeError` | `(code:int, f:Function) → void` | Enregistre un handler d'erreur personnalisé |
| `run` | `() → void` | Démarre le serveur (bloquant) |

## Exemple complet

```ocara
import ocara.HTTPServer
import ocara.HTTPServerRequest
import ocara.IO

function main(): int {

    const server:HTTPServer = use HTTPServer()
    server.port(3000)
    server.workers(8)

    // Route GET /
    server.route("/", "GET", nameless(req:HTTPServerRequest): int {
        var name:mixed = req.param("name")
        if name equal "" {
            name = "Monde"
        }
        req.respondHeader("Content-Type", "text/plain; charset=utf-8")
        req.respond(200, `Bonjour ${name} !`)
        return 0
    })

    // Route POST /echo
    server.route("/echo", "POST", nameless(req:HTTPServerRequest): int {
        var body:string = req.body()
        req.respondHeader("Content-Type", "application/json")
        req.respond(200, `{"echo":"${body}"}`)
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
import ocara.HTTPServerRequest

class HomeController {
    public static method home(req:HTTPServerRequest): int {
        req.respond(200, "Page d'accueil")
        return 0
    }
}

function main(): int {
    const server:HTTPServer = use HTTPServer()
    server.port(8080)
    server.route("/", "GET", HomeController::home)
    server.run()
    return 0
}
```

> **Note** : `HomeController::home` (sans parenthèses) transmet un fat pointer vers la méthode. `HomeController::home()` appellerait la méthode immédiatement — ce n'est pas ce que l'on veut ici.

## Fichiers statiques

HTTPServer peut servir des fichiers statiques (HTML, CSS, JS, images, etc.) depuis un répertoire racine défini avec `rootPath()`.

### Fonctionnement

1. **Routes dynamiques en priorité** : si une route correspond au chemin demandé, le handler est appelé
2. **Fallback fichiers statiques** : si aucune route ne correspond et qu'un `root_path` est défini, le serveur cherche un fichier correspondant
3. **404 si aucun match** : ni route ni fichier → erreur 404

### Exemple

```ocara
import ocara.HTTPServer
import ocara.HTTPServerRequest
import ocara.IO

function main(): int {
    const server:HTTPServer = use HTTPServer()
    server.port(8080)
    server.rootPath("./public")

    // Route dynamique API
    server.route("/api/hello", "GET", nameless(req:HTTPServerRequest): int {
        req.respondHeader("Content-Type", "application/json")
        req.respond(200, `{"message":"Hello API"}`)
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

La méthode `workers(n)` définit le nombre de threads de traitement parallèle.

> **Valeur par défaut** : `4` workers (si `workers()` n'est pas appelé)

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
server.workers(32)  // Bon compromis pour cette charge
server.run()
```

## Notes de sécurité / concurrence

`workers` threads acceptent réellement les connexions en parallèle (voir « Gestion des connexions multiples » ci-dessus) — ce n'est pas une simulation de concurrence. **Mais l'exécution d'un handler (route ou page d'erreur) est sérialisée** : un verrou interne, propre à chaque serveur, garantit qu'un seul handler Ocara s'exécute à la fois, quel que soit le nombre de workers. Deux requêtes simultanées ne peuvent donc **jamais** exécuter le même handler — ou deux handlers différents — en même temps. Ce qui reste parallèle entre requêtes : la lecture de la requête (corps, en-têtes) et l'envoi de la réponse, avant/après l'appel au handler.

Conséquence pratique : **une variable capturée par un handler et partagée uniquement avec d'autres handlers n'a plus besoin d'être protégée par un `Mutex`** — la sérialisation l'empêche par construction.

```ocara
var hitCount:int = 0

// Sûr par défaut : deux requêtes simultanées sur /hits ne peuvent jamais
// exécuter ce handler en même temps — aucune incrémentation ne peut se
// perdre, sans Mutex explicite.
server.route("/hits", "GET", nameless(req:HTTPServerRequest): int {
    hitCount = hitCount + 1
    req.respond(200, `Visites : ${hitCount}`)
    return 0
})
```

**Ce qui reste un vrai risque de data race**, et nécessite toujours `ocara.Mutex` (voir [Mutex](Mutex.md)) : une variable capturée par un handler **ET** touchée en dehors de toute invocation de handler — typiquement par un `ocara.Thread` tournant en tâche de fond, qui n'est pas concerné par le verrou interne de `HTTPServer` (celui-ci ne protège que les appels *dans* `handle_request`, pas le reste du programme) :

```ocara
import ocara.HTTPServer
import ocara.HTTPServerRequest
import ocara.Mutex
import ocara.Thread

function main(): int {
    const server:HTTPServer = use HTTPServer()
    var counter:int = 0
    var lock:Mutex = use Mutex()   // `var`, pas `scoped` : capturée par le handler ET
                                     // par le thread de fond, doit survivre aux deux

    server.route("/counter", "GET", nameless(req:HTTPServerRequest): int {
        var current:int = 0
        lock.withLock(nameless(): void {
            current = counter
        })
        req.respond(200, `Compteur : ${current}`)
        return 0
    })

    // Tourne en dehors du verrou interne de HTTPServer : sans Mutex ici,
    // data race réel avec le handler ci-dessus.
    var bg:Thread = Thread::spawn(nameless(): void {
        lock.withLock(nameless(): void {
            counter = counter + 1
        })
    })

    server.run()
    return 0
}
```

`withLock` (plutôt que `lock()`/`unlock()` manuels) garantit le déverrouillage même si le code protégé lève une exception avant d'atteindre `unlock()` — sinon tout appel suivant qui tente de verrouiller reste bloqué indéfiniment (voir [Mutex](Mutex.md), section `withLock`).

**Ce qui N'A JAMAIS besoin de protection** : les données propres à UNE requête (`req`, tout ce que retournent `path`/`method`/`body`/`header`/`headers`/`query`/`param`/`params`) ne sont jamais partagées entre handlers — chaque requête a son propre `HTTPServerRequest`, alloué et libéré pour elle seule.

**Compromis assumé** : sérialiser l'invocation des handlers élimine la race par construction, au prix de perdre le parallélisme réel sur la logique métier elle-même (deux handlers ne tournent plus jamais en même temps, même s'ils ne partagent rien). C'est un choix de philosophie différent de `ocara.Thread`, qui reste "rapide par défaut, sûr sur demande (`Mutex`)" — voir `docs/roadmap.d/runtime-httpserver-race-condition.md` pour la justification complète de ce compromis.

## Note historique

Avant `HTTPServerRequest`, un handler recevait un `req:int` opaque (un pointeur déguisé en entier) et lisait/écrivait la requête via des méthodes **statiques** (`HTTPServer::path(req)`, `HTTPServer::respond(req, ...)`, etc.). Rien dans `req:int` n'indiquait qu'il s'agissait d'une requête HTTP — un appelant pouvait y passer n'importe quel entier sans que le typage ne le retienne. `HTTPServerRequest` est un remplacement **cassant, sans période de coexistence** : `req:int` ne compile plus. Voir `docs/roadmap.d/stdlib-httpserver-request-object.md` (désormais clos) pour l'historique complet de cette décision.
