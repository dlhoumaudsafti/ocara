# `ocara.JSON` — Classe builtin

> Classe de sérialisation et désérialisation JSON.  
> Toutes les méthodes sont **statiques** : elles s'appellent via `JSON::<méthode>(args)`.  
> Les méthodes sont également disponibles comme **méthodes d'instance** sur les types compatibles.

---

## Import

```ocara
import ocara.JSON        // importe uniquement JSON
import ocara.*           // importe toutes les classes builtins
```

---

## Référence des méthodes

### `JSON::encode(data)` → `string`

Encode un array ou une map en JSON.

| Paramètre | Type | Description |
|-----------|------|-------------|
| `data` | `array` ou `map` | Données à encoder en JSON |

```ocara
import ocara.JSON

var users:map<string, mixed> = {
    "name": "Alice",
    "age": 30,
    "active": true
}

scoped json:string = JSON::encode(users)
IO::writeln(json)
// → {"name":"Alice","age":30,"active":true}

// Avec un tableau
var items:mixed[] = [1, 2, "hello", true]
scoped arr_json:string = JSON::encode(items)
IO::writeln(arr_json)
// → [1,2,"hello",true]
```

**Méthode d'instance sur array/map :**

```ocara
import ocara.JSON

var config:map<string, int> = {"port": 8080, "timeout": 5000}
scoped json:string = config.encode()  // ✅ Appel direct sur la map
IO::writeln(json)
// → {"port":8080,"timeout":5000}

var numbers:int[] = [1, 2, 3, 4, 5]
scoped arr:string = numbers.encode()  // ✅ Appel direct sur l'array
IO::writeln(arr)
// → [1,2,3,4,5]
```

**Règles :**
- Supporte les types : `int`, `string`, `bool`, `null`, arrays imbriqués, maps imbriquées
- Les clés de map sont toujours converties en strings
- Les valeurs `float` ne sont pas encore supportées (seront ajoutées dans v0.2.0)

---

### `JSON::decode(json)` → `mixed`

Décode une string JSON en structure Ocara (array ou map).

| Paramètre | Type | Description |
|-----------|------|-------------|
| `json` | `string` | String JSON à décoder |

```ocara
import ocara.JSON

scoped json_str:string = `{"name":"Bob","age":25}`
var data:mixed = JSON::decode(json_str)

// data est une map<string, mixed>
// Accès aux valeurs
var m:map<string, mixed> = data
IO::writeln(m["name"])  // → Bob
IO::writeln(m["age"])   // → 25

// Décoder un tableau
scoped arr_json:string = `[10, 20, 30]`
var arr:mixed = JSON::decode(arr_json)
// arr est un mixed[] (array)
```

**Méthode d'instance sur string :**

```ocara
import ocara.JSON

scoped json:string = `{"status":"ok","code":200}`
var result:mixed = json.decode()  // ✅ Appel direct sur la string
IO::writeln(result)
```

**Règles :**
- Retourne `null` (0) si le JSON est invalide
- Les objets JSON deviennent des `map<string, mixed>`
- Les tableaux JSON deviennent des `mixed[]`
- Les nombres JSON deviennent des `int`
- Les booléens et `null` JSON sont préservés

---

### `JSON::pretty(json)` → `string`

Formatte un JSON avec indentation pour améliorer la lisibilité.

| Paramètre | Type | Description |
|-----------|------|-------------|
| `json` | `string` | String JSON à formatter |

```ocara
import ocara.JSON

scoped compact:string = `{"name":"Alice","age":30,"tags":["dev","admin"]}`
scoped pretty:string = JSON::pretty(compact)
IO::writeln(pretty)
// → {
//     "name": "Alice",
//     "age": 30,
//     "tags": [
//       "dev",
//       "admin"
//     ]
//   }
```

**Méthode d'instance sur string :**

```ocara
import ocara.JSON

scoped json:string = `{"x":1,"y":2}`
scoped formatted:string = json.pretty()  // ✅ Appel direct
IO::writeln(formatted)
```

**Règles :**
- Utilise une indentation de 2 espaces
- Retourne la string originale si le JSON est invalide
- Idéal pour le debugging et les logs

---

### `JSON::minimize(json)` → `string`

Minifie un JSON en supprimant les espaces et retours à la ligne.

| Paramètre | Type | Description |
|-----------|------|-------------|
| `json` | `string` | String JSON à minifier |

```ocara
import ocara.JSON

scoped formatted:string = `{
  "name": "Alice",
  "age": 30
}`

scoped mini:string = JSON::minimize(formatted)
IO::writeln(mini)
// → {"name":"Alice","age":30}
```

**Méthode d'instance sur string :**

```ocara
import ocara.JSON

scoped json:string = `{
  "status": "ok",
  "data": [1, 2, 3]
}`

scoped compact:string = json.minimize()  // ✅ Appel direct
IO::writeln(compact)
// → {"status":"ok","data":[1,2,3]}
```

**Règles :**
- Supprime tous les espaces et retours à la ligne non nécessaires
- Retourne la string originale si le JSON est invalide
- Utile pour réduire la taille des données transmises

---

## Méthodes d'instance — Résumé

Les méthodes JSON peuvent être appelées directement sur les types compatibles **sans prefixe de classe et sans import** :

| Type | Méthodes disponibles | Exemple |
|------|----------------------|---------|
| `array` (T[]) | `.encode()` | `[1,2,3].encode()` |
| `map` (map<K,V>) | `.encode()` | `{"x":1}.encode()` |
| `string` | `.decode()`, `.pretty()`, `.minimize()` | `json.decode()` |

**Aucun import requis pour les méthodes d'instance :**

```ocara
// Pas besoin d'import pour les méthodes d'instance
var data:map<string, int> = {"count": 42}
scoped json:string = data.encode()  // ✅ Fonctionne sans import
```

**Import requis uniquement pour les méthodes statiques :**

Les méthodes statiques JSON nécessitent un import explicite :

```ocara
import ocara.JSON  // ← OBLIGATOIRE pour JSON::encode() etc.

var data:map<string, int> = {"count": 42}
scoped json:string = JSON::encode(data)  // ✅ Fonctionne avec import
```

Sans import :
```ocara
var data:map<string, int> = {"count": 42}
scoped json:string = JSON::encode(data)  // ❌ ERREUR: unknown builtin module 'ocara.JSON'
```

---

## Exemples complets

### Encoder et décoder des structures complexes

```ocara
import ocara.IO
import ocara.JSON

function main(): int {
    // ── Encoder une structure imbriquée ──────────────────────────────────
    var user:map<string, mixed> = {
        "name": "Alice",
        "age": 30,
        "roles": ["admin", "dev"],
        "settings": {"theme": "dark", "lang": "fr"}
    }
    
    scoped json:string = user.encode()
    IO::writeln("JSON encodé:")
    IO::writeln(json.pretty())
    
    // ── Décoder et accéder aux données ───────────────────────────────────
    var decoded:mixed = json.decode()
    var user_map:map<string, mixed> = decoded
    
    IO::writeln(`Nom: ${user_map["name"]}`)
    IO::writeln(`Age: ${user_map["age"]}`)
    
    return 0
}
```

### API REST avec JSON

```ocara
import ocara.HTTPServer
import ocara.JSON

function handle_api(req:int): int {
    // Lire le body de la requête et le décoder
    scoped body:string = HTTPServer::reqBody(req)
    var data:mixed = body.decode()
    
    // Traiter les données...
    var response:map<string, mixed> = {
        "status": "ok",
        "received": data
    }
    
    // Encoder la réponse et l'envoyer
    HTTPServer::setRespHeader(req, "Content-Type", "application/json")
    HTTPServer::respond(req, 200, response.encode())
    return 0
}
```

---

## Limitations actuelles (v0.1.0)

- ⚠️ **Entier 1** : La valeur `1` est parfois interprétée comme booléen `true` dans l'encodage (sera corrigé dans v0.2.0)
- ❌ **Float** : Les valeurs `float` ne sont pas encore supportées dans l'encodage/décodage (prévu pour v0.2.0)
- ❌ **Types custom** : Les objets de classes utilisateur ne peuvent pas être encodés directement (utiliser des maps intermédiaires)
- ✅ **Performance** : L'implémentation utilise `serde_json` (Rust) pour des performances optimales

---

## Notes techniques

### Représentation interne

- Les arrays JSON → `mixed[]` (array Ocara)
- Les objets JSON → `map<string, mixed>` (map Ocara)
- Les nombres JSON → `int` (pas de distinction float pour l'instant)
- Les strings JSON → `string` (avec échappement Unicode)
- Les booleans JSON → `bool` (true/false)
- Le null JSON → `null` (valeur 0)

### Gestion des erreurs

Les fonctions JSON ne lèvent **aucune exception**. En cas d'erreur :
- `decode()` retourne `null` (0)
- `pretty()` et `minimize()` retournent la string originale inchangée
- `encode()` retourne une string vide `""` pour les types non supportés

Pour une gestion robuste :

```ocara
import ocara.JSON

scoped json:string = `{"malformed": `
var result:mixed = json.decode()

if result is null {
    IO::writeln("Erreur : JSON invalide")
} else {
    IO::writeln("JSON décodé avec succès")
}
```
