# `ocara.Map` — Classe builtin

> Classe de manipulation des maps (tableaux associatifs clé/valeur).  
> Toutes les méthodes sont **statiques** : elles s'appellent via `Map::<méthode>(args)`.  
> Fonctionne avec n'importe quel type de clé et de valeur.

---

## Import

```ocara
import ocara.Map        // importe uniquement Map
import ocara.*          // importe toutes les classes builtins
```

---

## Référence des méthodes

### `Map::size(m)` → `int`

Retourne le nombre d'entrées de la map.

```ocara
var m:map = {"a": 1, "b": 2, "c": 3}
Map::size(m)    // → 3
Map::size({})   // → 0
```

---

### `Map::isEmpty(m)` → `bool`

Retourne `true` si la map ne contient aucune entrée.

```ocara
Map::isEmpty({})              // → true
Map::isEmpty({"x": 1})       // → false
```

---

### `Map::has(m, key)` → `bool`

Retourne `true` si la clé `key` existe dans la map.

```ocara
var m:map = {"alice": 95, "bob": 72}
Map::has(m, "alice")    // → true
Map::has(m, "david")    // → false
```

---

### `Map::get(m, key)` → `mixed`

Retourne la valeur associée à `key`.  
Si la clé est absente, le comportement dépend du runtime (retourne `0` / chaîne vide par défaut).  
Toujours vérifier avec `Map::has` avant si la présence n'est pas garantie.

```ocara
var m:map = {"prix": 42, "qte": 3}
scoped p:int = Map::get(m, "prix")   // → 42
```

---

### `Map::set(m, key, val)` → `void`

Insère ou **met à jour** l'entrée `key` → `val` dans la map.  
Modifie la map en place.

```ocara
var m:map = {"a": 1}
Map::set(m, "b", 2)    // insère  → {"a": 1, "b": 2}
Map::set(m, "a", 99)   // mise à jour → {"a": 99, "b": 2}
```

---

### `Map::remove(m, key)` → `void`

Supprime l'entrée associée à `key`. Sans effet si la clé est absente.  
Modifie la map en place.

```ocara
var m:map = {"a": 1, "b": 2}
Map::remove(m, "a")
// m est maintenant {"b": 2}
```

---

### `Map::keys(m)` → `array<mixed>`

Retourne un tableau contenant toutes les clés de la map.  
L'ordre n'est pas garanti.

```ocara
var m:map = {"x": 10, "y": 20, "z": 30}
scoped cles:array<string> = Map::keys(m)
// → ["x", "y", "z"] (ordre quelconque)
```

---

### `Map::values(m)` → `array<mixed>`

Retourne un tableau contenant toutes les valeurs de la map.  
L'ordre correspond à celui de `Map::keys`.

```ocara
var m:map = {"x": 10, "y": 20}
scoped vals:array<int> = Map::values(m)
// → [10, 20]
```

---

### `Map::merge(a, b)` → `map`

Retourne une **nouvelle** map contenant toutes les entrées de `a` et `b`.  
En cas de clé commune, la valeur de `b` **écrase** celle de `a`.

```ocara
var base:map = {"timeout": 30, "retry": 3}
var extra:map = {"timeout": 60, "debug": 1}

scoped config:map = Map::merge(base, extra)
Map::get(config, "timeout")   // → 60  (écrasé par extra)
Map::get(config, "retry")     // → 3   (conservé de base)
Map::get(config, "debug")     // → 1   (ajouté depuis extra)
```

---

### `Map::forEach(m, callback)` → `void`

Appelle `callback` pour chaque entrée de la map, avec la clé et la valeur.

**Paramètres** :
- `m` : la map à parcourir
- `callback` : `nameless(key:mixed, value:mixed): void` — appelé une fois par entrée

```ocara
const scores:map<string, mixed> = {"Alice": 95, "Bob": 82}

Map::forEach(scores, nameless(key:mixed, value:mixed): void {
    IO::writeln(`${key} : ${value}`)
})
// Alice : 95
// Bob : 82
```

Le callback peut capturer et modifier des variables du scope englobant (comme toute closure `nameless`) :

```ocara
var lines:array<string> = []
Map::forEach(scores, nameless(key:mixed, value:mixed): void {
    Array::push(lines, key + " -> " + value)
})
```

> **Limite connue** : `value` est typé `mixed` (comme `Map::get`). Le concaténer à une `string` (`+` ou template) fonctionne, mais une **opération arithmétique** directe (`total + value`) sur une valeur `mixed` produit aujourd'hui un résultat incorrect, y compris après un narrowing `if value is int { ... }` — bug de compilateur indépendant de `forEach` (voir `docs/roadmap.d/langage-mixed-arithmetic.md`). En attendant, convertir explicitement via une string (`Convert::strToInt(\`${value}\`)`) plutôt qu'une addition directe.

---

## Combinaisons courantes

```ocara
import ocara.Map
import ocara.Array

function main(): int {

    // Initialisation et alimentation dynamique
    var compteurs:map = {}
    var mots:array<string> = ["foo", "bar", "foo", "baz", "foo", "bar"]

    for mot in mots {
        if Map::has(compteurs, mot) {
            scoped c:int = Map::get(compteurs, mot)
            Map::set(compteurs, mot, c + 1)
        }
        Map::set(compteurs, mot, 1)
    }

    // Afficher les résultats
    for cle in Map::keys(compteurs) {
        write(`${cle} : ${Map::get(compteurs, cle)}`)
    }

    // Fusion de configurations
    var defaults:map = {"lang": "fr", "theme": "light", "limit": 20}
    var user_prefs:map = {"theme": "dark", "limit": 50}
    scoped final_cfg:map = Map::merge(defaults, user_prefs)

    write(`theme : ${Map::get(final_cfg, "theme")}`)   // dark
    write(`lang  : ${Map::get(final_cfg, "lang")}`)    // fr

    // Vérification avant accès
    var m:map = {}
    if Map::isEmpty(m) {
        write("aucune donnée")
    }

    return 0
}
```

---

## Gestion d'erreurs

Certaines méthodes Map peuvent lever une `MapException` en cas d'erreur.

### Codes d'erreur MapException

| Code | Nom | Opération | Description |
|------|------|-----------|-------------|
| 101 | `KEY_NOT_FOUND` | `Map::get()` | Clé inexistante dans la map |

### Exemples de gestion d'erreurs

#### Gestion générique

```ocara
import ocara.Map
import ocara.IO

function main(): int {
    var config:map<string,string> = {"env": "prod"}
    
    try {
        var db:string = Map::get(config, "database")
        IO::writeln(`Database: ${db}`)
    } on e is MapException {
        IO::writeln(`Map error: ${e.message}`)
        IO::writeln(`Code: ${e.code}`)
    }
    
    return 0
}
```

#### Gestion avec code d'erreur spécifique

```ocara
import ocara.Map
import ocara.IO

function safe_get(m:map<string,string>, key:string): string {
    try {
        return Map::get(m, key)
    } on e is MapException {
        if e.code equal 101 {
            IO::writeln(`Key '${key}' not found, using default`)
            return ""
        } else {
            IO::writeln(`Unexpected error: ${e.message}`)
            return ""
        }
    }
}

function main(): int {
    var settings:map<string,string> = {"mode": "debug"}
    var host:string = safe_get(settings, "host")
    IO::writeln(`Host: ${host}`)
    return 0
}
```

#### Vérification avec has() avant get()

```ocara
import ocara.Map
import ocara.IO

function main(): int {
    var data:map<string,int> = {"count": 42}
    
    // Approche sûre : vérifier d'abord
    if Map::has(data, "count") {
        var count:int = Map::get(data, "count")
        IO::writeln(`Count: ${count}`)
    } else {
        IO::writeln("Key 'count' not found")
    }
    
    // Ou utiliser try/on
    try {
        var total:int = Map::get(data, "total")
        IO::writeln(`Total: ${total}`)
    } on e is MapException {
        IO::writeln("Key 'total' not found")
    }
    
    return 0
}
```

#### Catch générique (sans type)

```ocara
import ocara.Map
import ocara.IO

function main(): int {
    var users:map<string,string> = {"admin": "alice"}
    
    try {
        var user:string = Map::get(users, "guest")
        IO::writeln(`User: ${user}`)
    } on e {
        // Capture toute exception
        IO::writeln(`Exception: ${e.message}`)
        IO::writeln(`Source: ${e.source}`)
    }
    
    return 0
}
```

#### Multiple handlers avec filtrage par type

```ocara
import ocara.Map
import ocara.File
import ocara.MapException
import ocara.FileException
import ocara.IO

function load_config(path:string): map<string,string> {
    var content:string = File::read(path)
    // ... parse content into map ...
    var cfg:map<string,string> = {"version": "1.0"}
    return cfg
}

function main(): int {
    try {
        var config:map<string,string> = load_config("/config.json")
        var dbUrl:string = Map::get(config, "database_url")
        IO::writeln(`DB: ${dbUrl}`)
    } on e is MapException {
        IO::writeln(`Config key missing (code ${e.code}): ${e.message}`)
    } on e is FileException {
        IO::writeln(`Config file error (code ${e.code}): ${e.message}`)
    }
    
    return 0
}
```

### Format des messages d'exception

Les messages d'exception sont en anglais et incluent la clé recherchée :
- `Key not found: database_url`
- `Key not found: 123` (pour les clés numériques)

**Notes :**
- `Map::has()` ne lève jamais d'exception (retourne true/false)
- `Map::set()` ne lève jamais d'exception (crée ou met à jour la clé)
- `Map::remove()` ne lève jamais d'exception (même si la clé n'existe pas)
- `Map::size()`, `Map::isEmpty()` ne lèvent jamais d'exception
- `Map::keys()`, `Map::values()` ne lèvent jamais d'exception (retournent un tableau vide si la map est vide)
- `Map::merge()` ne lève jamais d'exception

**Conseil :** Pour éviter les exceptions, utilisez `Map::has()` pour vérifier l'existence d'une clé avant d'appeler `Map::get()`.

---

## Conventions runtime

| Méthode Ocara      | Symbole runtime C | Params Cranelift        | Retour  |
|--------------------|-------------------|-------------------------|---------|
| `Map::size`        | `Map_size`        | `I64`                   | `I64`   |
| `Map::has`         | `Map_has`         | `I64, I64`              | `I64`   |
| `Map::get`         | `Map_get`         | `I64, I64`              | `I64`   |
| `Map::set`         | `Map_set`         | `I64, I64, I64`         | —       |
| `Map::remove`      | `Map_remove`      | `I64, I64`              | —       |
| `Map::keys`        | `Map_keys`        | `I64`                   | `I64`   |
| `Map::values`      | `Map_values`      | `I64`                   | `I64`   |
| `Map::merge`       | `Map_merge`       | `I64, I64`              | `I64`   |
| `Map::isEmpty`     | `Map_isEmpty`    | `I64`                   | `I64`   |

> **Note** : les primitives internes `__map_new`, `__map_get`, `__map_set`, `__map_foreach` sont utilisées par le compilateur pour la syntaxe `{"k": v}` et les boucles `for k in map`. La classe `Map` builtin fournit une API de haut niveau au-dessus.

---

## Voir aussi

- [examples/builtins/map.oc](../../examples/builtins/map.oc) — exemple complet exécutable
- [docs/EBNF.md](../EBNF.md) — grammaire formelle
