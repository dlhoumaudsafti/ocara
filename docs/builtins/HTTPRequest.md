# HTTPRequest

Classe builtin `ocara.HTTPRequest` — requêtes HTTP/HTTPS avec gestion des en-têtes, codes de statut et corps de réponse.

Toutes les méthodes sont déclarées **statiques**, mais utilisables aussi en **syntaxe d'instance** (`req.send()`, `res.status()`...) — les deux formes appellent exactement la même fonction. `req`/`res` sont des types nommés RÉELS (`HTTPRequest`/`HTTPResponse`), pas de simples `int` : ils peuvent être déclarés `scoped`/`consumed` et sont alors fermés automatiquement en fin de bloc (voir [Libération](#libération) plus bas).

```ocara
import ocara.HTTPRequest
import ocara.HTTPResponse
// ou
import ocara.*
```

---

## Construction & configuration

### `HTTPRequest::new(url: string) → HTTPRequest`
Crée une nouvelle requête HTTP vers `url`. Retourne un handle de requête.
Méthode par défaut : `GET`.

```ocara
scoped req:HTTPRequest = HTTPRequest::new("https://api.example.com/resource")
```

### `HTTPRequest::setMethod(req: HTTPRequest, method: string) → void`
Définit la méthode HTTP : `"GET"`, `"POST"`, `"PUT"`, `"DELETE"`, `"PATCH"`, `"HEAD"`, `"OPTIONS"`.

```ocara
req.setMethod("POST")
```

### `HTTPRequest::setHeader(req: HTTPRequest, name: string, value: string) → void`
Ajoute ou remplace un en-tête de requête.

```ocara
req.setHeader("Content-Type", "application/json")
req.setHeader("Authorization", "Bearer token")
```

### `HTTPRequest::setBody(req: HTTPRequest, body: string) → void`
Définit le corps de la requête (JSON, form-data, texte brut…).

```ocara
req.setBody("{\"key\": \"value\"}")
```

### `HTTPRequest::setTimeout(req: HTTPRequest, ms: int) → void`
Délai maximum en millisecondes avant abandon de la connexion.

```ocara
req.setTimeout(5000)  // 5 secondes
```

---

## Exécution

### `HTTPRequest::send(req: HTTPRequest) → HTTPResponse`
Envoie la requête et retourne un handle de réponse. Bloquant.

```ocara
scoped res:HTTPResponse = req.send()
```

---

## Lecture de la réponse

### `HTTPRequest::status(res: HTTPResponse) → int`
Code de statut HTTP (`200`, `201`, `404`, `500`…).

### `HTTPRequest::body(res: HTTPResponse) → string`
Corps brut de la réponse (JSON, HTML, texte…).

### `HTTPRequest::header(res: HTTPResponse, name: string) → string`
Valeur d'un en-tête de réponse. Retourne `""` si absent.

```ocara
scoped ct:string = res.header("Content-Type")
```

### `HTTPRequest::headers(res: HTTPResponse) → map<string, string>`
Tous les en-têtes de réponse sous forme de map.

### `HTTPRequest::ok(res: HTTPResponse) → bool`
`true` si le code de statut est entre `200` et `299` inclus.

### `HTTPRequest::isError(res: HTTPResponse) → bool`
`true` si une erreur réseau ou un timeout s'est produit (indépendamment du code HTTP).

### `HTTPRequest::error(res: HTTPResponse) → string`
Message d'erreur réseau. Retourne `""` si la connexion a réussi.

---

## Raccourcis

Ces méthodes créent, configurent et envoient la requête en une seule étape — statiques uniquement (pas de receveur avant l'envoi).

| Méthode | Signature | Description |
|---|---|---|
| `get` | `(url: string) → HTTPResponse` | Requête GET |
| `post` | `(url: string, body: string) → HTTPResponse` | Requête POST |
| `put` | `(url: string, body: string) → HTTPResponse` | Requête PUT |
| `delete` | `(url: string) → HTTPResponse` | Requête DELETE |
| `patch` | `(url: string, body: string) → HTTPResponse` | Requête PATCH |

```ocara
scoped res:HTTPResponse = HTTPRequest::get("https://api.example.com/users")
scoped res2:HTTPResponse = HTTPRequest::post("https://api.example.com/users", "{\"name\":\"Alice\"}")
scoped res3:HTTPResponse = HTTPRequest::delete("https://api.example.com/users/42")
```

---

## Libération

Chaque `req` (créé par `new`) et chaque `res` (créé par `send`/`get`/`post`/`put`/
`delete`/`patch`) possède un handle natif — ce runtime n'a pas de ramasse-miettes.

- **`scoped`/`consumed`** : fermé **automatiquement** en fin de bloc si jamais fermé manuellement avant (comme `Mutex`/`SQLite`) — c'est la façon recommandée de les déclarer.
- **`var`/`const`** : jamais fermé automatiquement ; s'il est prouvé qu'il ne s'échappe jamais et n'est jamais fermé manuellement, le compilateur **rejette** la déclaration (fuite de handle natif garantie — voir diagnostic E28).

### `HTTPRequest::close(req: HTTPRequest) → void`
Ferme un handle de requête créé par `new`. Un second appel sur le même handle est rejeté à la compilation (E25).

### `HTTPRequest::closeResponse(res: HTTPResponse) → void`
Ferme un handle de réponse créé par `send`/`get`/`post`/`put`/`delete`/`patch`.
Deux fonctions distinctes car `req` et `res` sont deux structures natives différentes.

```ocara
scoped req:HTTPRequest = HTTPRequest::new("https://api.example.com/resource")
req.setMethod("POST")
scoped res:HTTPResponse = req.send()
IO::writeln(res.body())
req.close()
res.closeResponse()
```

> **Attention** : comme pour `SQLite::close()`, tout appel sur un handle après
> sa fermeture est un comportement non défini (mémoire déjà libérée) — fermer
> uniquement après la dernière lecture. Une fermeture manuelle suivie d'une
> seconde fermeture (manuelle ou automatique en fin de `scoped`) est détectée
> à la compilation, pas besoin de s'en soucier soi-même une fois fermé.

---

## Exemples complets

### GET simple
```ocara
import ocara.HTTPRequest
import ocara.HTTPResponse
import ocara.IO

scoped res:HTTPResponse = HTTPRequest::get("https://api.example.com/users")

if res.ok() {
    IO::writeln(res.body())
} else {
    IO::writeln(`Erreur HTTP ${res.status()}`)
}
```

### POST JSON avec en-têtes
```ocara
import ocara.HTTPRequest
import ocara.HTTPResponse
import ocara.IO

scoped req:HTTPRequest = HTTPRequest::new("https://api.example.com/users")
req.setMethod("POST")
req.setHeader("Content-Type", "application/json")
req.setHeader("Authorization", "Bearer mon-token")
req.setBody("{\"name\": \"Alice\", \"age\": 30}")
req.setTimeout(10000)

scoped res:HTTPResponse = req.send()

IO::writeln(`Status : ${res.status()}`)
IO::writeln(res.body())
```

### Gestion d'erreur réseau
```ocara
import ocara.HTTPRequest
import ocara.HTTPResponse
import ocara.IO

scoped res:HTTPResponse = HTTPRequest::get("https://hote-inexistant.local/api")

if res.isError() {
    IO::writeln(`Erreur réseau : ${res.error()}`)
} else {
    IO::writeln(`Status : ${res.status()}`)
    IO::writeln(res.body())
}
```

### Lecture des en-têtes de réponse
```ocara
import ocara.HTTPRequest
import ocara.HTTPResponse
import ocara.IO

scoped res:HTTPResponse = HTTPRequest::get("https://api.example.com/info")
scoped hdrs:map<string, string> = res.headers()

IO::writeln(`Content-Type : ${res.header("Content-Type")}`)
IO::writeln(`X-RateLimit-Remaining : ${res.header("X-RateLimit-Remaining")}`)
```

---

## Codes HTTP courants

| Code | Signification |
|---|---|
| `200` | OK |
| `201` | Created |
| `204` | No Content |
| `301` / `302` | Redirection |
| `400` | Bad Request |
| `401` | Unauthorized |
| `403` | Forbidden |
| `404` | Not Found |
| `429` | Too Many Requests |
| `500` | Internal Server Error |
| `503` | Service Unavailable |

---

## Symboles runtime

| Méthode Ocara | Symbole C runtime | Receveur (syntaxe d'instance) |
|---|---|---|
| `new` | `HTTPRequest_new` | — (statique uniquement) |
| `setMethod` | `HTTPRequest_setMethod` | `HTTPRequest` |
| `setHeader` | `HTTPRequest_setHeader` | `HTTPRequest` |
| `setBody` | `HTTPRequest_setBody` | `HTTPRequest` |
| `setTimeout` | `HTTPRequest_setTimeout` | `HTTPRequest` |
| `send` | `HTTPRequest_send` | `HTTPRequest` |
| `status` | `HTTPRequest_status` | `HTTPResponse` |
| `body` | `HTTPRequest_body` | `HTTPResponse` |
| `header` | `HTTPRequest_header` | `HTTPResponse` |
| `headers` | `HTTPRequest_headers` | `HTTPResponse` |
| `ok` | `HTTPRequest_ok` | `HTTPResponse` |
| `isError` | `HTTPRequest_isError` | `HTTPResponse` |
| `error` | `HTTPRequest_error` | `HTTPResponse` |
| `get` | `HTTPRequest_get` | — (statique uniquement) |
| `post` | `HTTPRequest_post` | — (statique uniquement) |
| `put` | `HTTPRequest_put` | — (statique uniquement) |
| `delete` | `HTTPRequest_delete` | — (statique uniquement) |
| `patch` | `HTTPRequest_patch` | — (statique uniquement) |
| `close` | `HTTPRequest_close` | `HTTPRequest` |
| `closeResponse` | `HTTPRequest_closeResponse` | `HTTPResponse` |

> `HTTPResponse` est un type nommé opaque sans méthode propre : toutes les méthodes ci-dessus restent déclarées sur `HTTPRequest` — la colonne "Receveur" indique seulement quel type peut les appeler en syntaxe d'instance (`res.status()` cherche `status` du côté `HTTPRequest` pour le compte de `HTTPResponse`, mais `res.send()`/`req.status()` — mélanger les deux — sont rejetés à la compilation).

## Voir aussi

- [Mutex](Mutex.md) — même discipline `scoped`/`consumed` + fermeture manuelle pour un handle natif.
- [SQLite](SQLite.md) — même patron `close()` géré par l'analyse de possession.
