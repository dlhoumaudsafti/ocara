# HTTPRequest

Classe builtin `ocara.HTTPRequest` — requêtes HTTP/HTTPS avec gestion des en-têtes, codes de statut et corps de réponse.

Toutes les méthodes sont **statiques**. Les handles `req` et `res` sont des valeurs opaques de type `int` gérées par le runtime.

```ocara
import ocara.HTTPRequest
// ou
import ocara.*
```

---

## Construction & configuration

### `HTTPRequest::new(url: string) → int`
Crée une nouvelle requête HTTP vers `url`. Retourne un handle de requête.  
Méthode par défaut : `GET`.

```ocara
scoped req = HTTPRequest::new("https://api.example.com/resource")
```

### `HTTPRequest::set_method(req: int, method: string) → void`
Définit la méthode HTTP : `"GET"`, `"POST"`, `"PUT"`, `"DELETE"`, `"PATCH"`, `"HEAD"`, `"OPTIONS"`.

```ocara
HTTPRequest::set_method(req, "POST")
```

### `HTTPRequest::set_header(req: int, name: string, value: string) → void`
Ajoute ou remplace un en-tête de requête.

```ocara
HTTPRequest::set_header(req, "Content-Type", "application/json")
HTTPRequest::set_header(req, "Authorization", "Bearer token")
```

### `HTTPRequest::set_body(req: int, body: string) → void`
Définit le corps de la requête (JSON, form-data, texte brut…).

```ocara
HTTPRequest::set_body(req, "{\"key\": \"value\"}")
```

### `HTTPRequest::set_timeout(req: int, ms: int) → void`
Délai maximum en millisecondes avant abandon de la connexion.

```ocara
HTTPRequest::set_timeout(req, 5000)  // 5 secondes
```

---

## Exécution

### `HTTPRequest::send(req: int) → int`
Envoie la requête et retourne un handle de réponse. Bloquant.

```ocara
scoped res = HTTPRequest::send(req)
```

---

## Lecture de la réponse

### `HTTPRequest::status(res: int) → int`
Code de statut HTTP (`200`, `201`, `404`, `500`…).

### `HTTPRequest::body(res: int) → string`
Corps brut de la réponse (JSON, HTML, texte…).

### `HTTPRequest::header(res: int, name: string) → string`
Valeur d'un en-tête de réponse. Retourne `""` si absent.

```ocara
scoped ct = HTTPRequest::header(res, "Content-Type")
```

### `HTTPRequest::headers(res: int) → map<string, string>`
Tous les en-têtes de réponse sous forme de map.

### `HTTPRequest::ok(res: int) → bool`
`true` si le code de statut est entre `200` et `299` inclus.

### `HTTPRequest::is_error(res: int) → bool`
`true` si une erreur réseau ou un timeout s'est produit (indépendamment du code HTTP).

### `HTTPRequest::error(res: int) → string`
Message d'erreur réseau. Retourne `""` si la connexion a réussi.

---

## Raccourcis

Ces méthodes créent, configurent et envoient la requête en une seule étape.

| Méthode | Signature | Description |
|---|---|---|
| `get` | `(url: string) → int` | Requête GET |
| `post` | `(url: string, body: string) → int` | Requête POST |
| `put` | `(url: string, body: string) → int` | Requête PUT |
| `delete` | `(url: string) → int` | Requête DELETE |
| `patch` | `(url: string, body: string) → int` | Requête PATCH |

```ocara
scoped res = HTTPRequest::get("https://api.example.com/users")
scoped res = HTTPRequest::post("https://api.example.com/users", "{\"name\":\"Alice\"}")
scoped res = HTTPRequest::delete("https://api.example.com/users/42")
```

---

## Exemples complets

### GET simple
```ocara
import ocara.HTTPRequest
import ocara.IO

scoped res = HTTPRequest::get("https://api.example.com/users")

if HTTPRequest::ok(res) {
    IO::writeln(HTTPRequest::body(res))
} else {
    IO::writeln(`Erreur HTTP ${HTTPRequest::status(res)}`)
}
```

### POST JSON avec en-têtes
```ocara
import ocara.HTTPRequest
import ocara.IO

scoped req = HTTPRequest::new("https://api.example.com/users")
HTTPRequest::set_method(req, "POST")
HTTPRequest::set_header(req, "Content-Type", "application/json")
HTTPRequest::set_header(req, "Authorization", "Bearer mon-token")
HTTPRequest::set_body(req, "{\"name\": \"Alice\", \"age\": 30}")
HTTPRequest::set_timeout(req, 10000)

scoped res = HTTPRequest::send(req)

IO::writeln(`Status : ${HTTPRequest::status(res)}`)
IO::writeln(HTTPRequest::body(res))
```

### Gestion d'erreur réseau
```ocara
import ocara.HTTPRequest
import ocara.IO

scoped res = HTTPRequest::get("https://hote-inexistant.local/api")

if HTTPRequest::is_error(res) {
    IO::writeln(`Erreur réseau : ${HTTPRequest::error(res)}`)
} else {
    IO::writeln(`Status : ${HTTPRequest::status(res)}`)
    IO::writeln(HTTPRequest::body(res))
}
```

### Lecture des en-têtes de réponse
```ocara
import ocara.HTTPRequest
import ocara.IO

scoped res = HTTPRequest::get("https://api.example.com/info")
scoped hdrs = HTTPRequest::headers(res)

IO::writeln(`Content-Type : ${HTTPRequest::header(res, "Content-Type")}`)
IO::writeln(`X-RateLimit-Remaining : ${HTTPRequest::header(res, "X-RateLimit-Remaining")}`)
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

| Méthode Ocara | Symbole C runtime |
|---|---|
| `new` | `HTTPRequest_new` |
| `set_method` | `HTTPRequest_set_method` |
| `set_header` | `HTTPRequest_set_header` |
| `set_body` | `HTTPRequest_set_body` |
| `set_timeout` | `HTTPRequest_set_timeout` |
| `send` | `HTTPRequest_send` |
| `status` | `HTTPRequest_status` |
| `body` | `HTTPRequest_body` |
| `header` | `HTTPRequest_header` |
| `headers` | `HTTPRequest_headers` |
| `ok` | `HTTPRequest_ok` |
| `is_error` | `HTTPRequest_is_error` |
| `error` | `HTTPRequest_error` |
| `get` | `HTTPRequest_get` |
| `post` | `HTTPRequest_post` |
| `put` | `HTTPRequest_put` |
| `delete` | `HTTPRequest_delete` |
| `patch` | `HTTPRequest_patch` |


## Voir aussi

- [examples/builtins/http.oc](../../examples/builtins/http.oc) — exemple complet exécutable
- [docs/EBNF.md](../EBNF.md) — grammaire formelle