# `ocara.Convert` — Classe builtin

> Classe de conversion entre types primitifs et structures de données.  
> Toutes les méthodes sont **statiques** : elles s'appellent via `Convert::<méthode>(args)`.

---

## Import

```ocara
import ocara.Convert        // importe uniquement Convert
import ocara.*              // importe toutes les classes builtins
```

---

## `string` → *

### `Convert::str_to_int(s)` → `int`

Convertit la chaîne `s` en entier. Retourne `0` si la conversion échoue.

```ocara
Convert::str_to_int("42")     // → 42
Convert::str_to_int("-7")     // → -7
Convert::str_to_int("abc")    // → 0
```

---

### `Convert::str_to_float(s)` → `float`

Convertit la chaîne `s` en décimal. Retourne `0.0` si la conversion échoue.

```ocara
Convert::str_to_float("3.14")   // → 3.14
Convert::str_to_float("42")     // → 42.0
Convert::str_to_float("abc")    // → 0.0
```

---

### `Convert::str_to_bool(s)` → `bool`

Retourne `true` si `s` vaut `"true"` ou `"1"` (insensible à la casse), `false` sinon.

```ocara
Convert::str_to_bool("true")    // → true
Convert::str_to_bool("1")       // → true
Convert::str_to_bool("false")   // → false
Convert::str_to_bool("0")       // → false
```

---

### `Convert::str_to_array(s, sep)` → `string[]`

Découpe `s` selon le séparateur `sep` et retourne un `string[]`.

```ocara
Convert::str_to_array("rust,ocara,web", ",")   // → ["rust", "ocara", "web"]
Convert::str_to_array("10 20 30", " ")          // → ["10", "20", "30"]
```

---

### `Convert::str_to_map(s, sep, kv)` → `map<string, string>`

Parse `s` en map clé/valeur.  
- `sep` : séparateur entre les paires  
- `kv`  : séparateur clé/valeur au sein d'une paire

```ocara
scoped m:map<string, string> = Convert::str_to_map("lang=fr,theme=dark", ",", "=")
// → {"lang": "fr", "theme": "dark"}

scoped m2:map<string, string> = Convert::str_to_map("x:10 y:20", " ", ":")
// → {"x": "10", "y": "20"}
```

---

## `int` → *

### `Convert::int_to_str(n)` → `string`

```ocara
Convert::int_to_str(42)    // → "42"
Convert::int_to_str(-7)    // → "-7"
```

### `Convert::int_to_float(n)` → `float`

```ocara
Convert::int_to_float(7)   // → 7.0
```

### `Convert::int_to_bool(n)` → `bool`

Retourne `false` si `n == 0`, `true` sinon.

```ocara
Convert::int_to_bool(0)    // → false
Convert::int_to_bool(1)    // → true
Convert::int_to_bool(-3)   // → true
```

---

## `float` → *

### `Convert::float_to_str(f)` → `string`

```ocara
Convert::float_to_str(3.14)   // → "3.14"
```

### `Convert::float_to_int(f)` → `int`

Troncature vers zéro (pas d'arrondi).

```ocara
Convert::float_to_int(9.99)    // → 9
Convert::float_to_int(-3.7)    // → -3
```

### `Convert::float_to_bool(f)` → `bool`

Retourne `false` si `f == 0.0`, `true` sinon.

```ocara
Convert::float_to_bool(0.0)    // → false
Convert::float_to_bool(1.5)    // → true
```

---

## `bool` → *

### `Convert::bool_to_str(b)` → `string`

```ocara
Convert::bool_to_str(true)    // → "true"
Convert::bool_to_str(false)   // → "false"
```

### `Convert::bool_to_int(b)` → `int`

```ocara
Convert::bool_to_int(true)    // → 1
Convert::bool_to_int(false)   // → 0
```

### `Convert::bool_to_float(b)` → `float`

```ocara
Convert::bool_to_float(true)    // → 1.0
Convert::bool_to_float(false)   // → 0.0
```

---

## `array` → *

### `Convert::array_to_str(arr, sep)` → `string`

Joint les éléments du tableau en une chaîne séparée par `sep`.  
Équivalent à `Array::join`.

```ocara
var t:string[] = ["rust", "ocara", "web"]
Convert::array_to_str(t, ", ")   // → "rust, ocara, web"
Convert::array_to_str(t, " | ")  // → "rust | ocara | web"
```

### `Convert::array_to_map(arr, kv)` → `map<string, string>`

Chaque élément du tableau doit être de la forme `"clé<kv>valeur"`.

```ocara
var pairs:string[] = ["lang=fr", "theme=dark", "debug=1"]
scoped m:map<string, string> = Convert::array_to_map(pairs, "=")
// → {"lang": "fr", "theme": "dark", "debug": "1"}
```

---

## `map` → *

### `Convert::map_to_str(m, sep, kv)` → `string`

Sérialise la map en chaîne. Inverse de `str_to_map`.

```ocara
var m:map<string, string> = {"lang": "fr", "theme": "dark"}
Convert::map_to_str(m, ",", "=")   // → "lang=fr,theme=dark"
```

### `Convert::map_keys_to_array(m)` → `string[]`

Retourne un tableau de toutes les clés. Équivalent à `Map::keys`.

```ocara
scoped cles:string[] = Convert::map_keys_to_array(m)
// → ["lang", "theme"]
```

### `Convert::map_values_to_array(m)` → `mixed[]`

Retourne un tableau de toutes les valeurs. Équivalent à `Map::values`.

```ocara
scoped vals:mixed[] = Convert::map_values_to_array(m)
// → ["fr", "dark"]
```

---

## Combinaisons courantes

```ocara
import ocara.Convert
import ocara.Array
import ocara.Map

function main(): int {

    // Lire un entier stocké en string, calculer, reconvertir
    var s:string  = "128"
    scoped n:int  = Convert::str_to_int(s)
    scoped r:string = Convert::int_to_str(n / 2)
    write(`Moitié de ${s} = ${r}`)   // 64

    // Parser une config inline
    scoped cfg:map<string, string> = Convert::str_to_map("debug=1,lang=fr,limit=50", ",", "=")
    scoped debug:bool = Convert::str_to_bool(Map::get(cfg, "debug"))
    scoped limit:int  = Convert::str_to_int(Map::get(cfg, "limit"))
    write(`debug=${debug}  limit=${limit}`)   // debug=true  limit=50

    // Sérialiser un tableau en CSV puis le reparseur
    var data:string[] = ["alice", "bob", "charlie"]
    scoped csv:string    = Convert::array_to_str(data, ",")
    scoped back:string[] = Convert::str_to_array(csv, ",")
    write(`roundtrip : ${Array::len(back)} éléments`)   // 3

    return 0
}
```

---

## Gestion d'erreurs

Certaines méthodes Convert peuvent lever une `ConvertException` en cas d'erreur.

### Codes d'erreur ConvertException

| Code | Nom | Opération | Description |
|------|------|-----------|-------------|
| 101 | `INVALID_INT` | `Convert::str_to_int()` | Impossible de convertir la chaîne en entier (format invalide) |
| 102 | `INVALID_FLOAT` | `Convert::str_to_float()` | Impossible de convertir la chaîne en flottant (format invalide) |

### Exemples de gestion d'erreurs

#### Conversion string vers int avec gestion d'erreur

```ocara
import ocara.Convert
import ocara.ConvertException
import ocara.IO

function main(): int {
    var input:string = "abc123"
    
    try {
        var num:int = Convert::str_to_int(input)
        IO::writeln(`Number: ${num}`)
    } on e is ConvertException {
        IO::writeln(`Conversion error: ${e.message}`)
        IO::writeln(`Code: ${e.code}`)
        if e.code == 101 {
            IO::writeln("Invalid integer format")
        }
    }
    
    return 0
}
```

#### Conversion string vers float avec gestion d'erreur

```ocara
import ocara.Convert
import ocara.ConvertException
import ocara.IO

function main(): int {
    var values:string[] = ["3.14", "2.71", "not_a_number", "1.41"]
    
    scoped i:int = 0
    scoped len:int = Array::len(values)
    while i < len {
        var s:string = values.get(i)
        try {
            var f:float = Convert::str_to_float(s)
            IO::writeln(`✓ '${s}' = ${f}`)
        } on e is ConvertException {
            IO::writeln(`✗ '${s}' - invalid format`)
        }
        i = i + 1
    }
    
    return 0
}
```

#### Fonction safe avec valeur par défaut

```ocara
import ocara.Convert
import ocara.ConvertException
import ocara.IO

function safe_str_to_int(s:string, default:int): int {
    try {
        return Convert::str_to_int(s)
    } on e is ConvertException {
        IO::writeln(`Warning: invalid int '${s}', using default ${default}`)
        return default
    }
}

function main(): int {
    var result1:int = safe_str_to_int("42", 0)
    IO::writeln(`Result 1: ${result1}`)
    
    var result2:int = safe_str_to_int("xyz", -1)
    IO::writeln(`Result 2: ${result2}`)
    
    return 0
}
```

#### Catch générique

```ocara
import ocara.Convert
import ocara.IO

function main(): int {
    var text:string = "hello"
    
    try {
        var num:int = Convert::str_to_int(text)
        IO::writeln(`Number: ${num}`)
    } on e {
        // Capture toute exception
        IO::writeln(`Exception: ${e.message}`)
        IO::writeln(`Source: ${e.source}`)
        IO::writeln(`Code: ${e.code}`)
    }
    
    return 0
}
```

#### Multiple conversions avec handlers

```ocara
import ocara.Convert
import ocara.ConvertException
import ocara.IO

function parse_config(line:string): void {
    // Format: "key=value"
    var parts:string[] = String::split(line, "=")
    
    if Array::len(parts) != 2 {
        IO::writeln("Invalid config line format")
        return
    }
    
    var key:string = parts.get(0)
    var val:string = parts.get(1)
    
    // Essayer différents types
    try {
        var n:int = Convert::str_to_int(val)
        IO::writeln(`${key} (int) = ${n}`)
        return
    } on e is ConvertException {
        // Pas un int, essayer float
    }
    
    try {
        var f:float = Convert::str_to_float(val)
        IO::writeln(`${key} (float) = ${f}`)
        return
    } on e is ConvertException {
        // Pas un float non plus, c'est une string
        IO::writeln(`${key} (string) = ${val}`)
    }
}

function main(): int {
    parse_config("port=8080")
    parse_config("rate=0.05")
    parse_config("name=MyApp")
    return 0
}
```

### Format des messages d'exception

Les messages d'exception sont en anglais et incluent la valeur problématique :
- `Cannot convert string to int: 'abc123'`
- `Cannot convert string to float: 'not_a_number'`

**Notes sur les conversions sûres :**
- `Convert::str_to_bool()` ne lève jamais d'exception (retourne false pour valeurs inconnues)
- `Convert::int_to_*()` ne lèvent jamais d'exception (conversions toujours possibles)
- `Convert::float_to_*()` ne lèvent jamais d'exception (troncature pour int, toujours convertible)
- `Convert::bool_to_*()` ne lèvent jamais d'exception (true=1/"true", false=0/"false")
- `Convert::array_to_*()` ne lèvent jamais d'exception
- `Convert::map_to_*()` ne lèvent jamais d'exception

**Seules `str_to_int()` et `str_to_float()` peuvent lever des exceptions** car elles nécessitent un format spécifique.

---

## Conventions runtime

| Méthode Ocara                     | Symbole runtime C                    | Params       | Retour  |
|-----------------------------------|--------------------------------------|--------------|---------|
| `Convert::str_to_int`             | `Convert_str_to_int`                 | `I64`        | `I64`   |
| `Convert::str_to_float`           | `Convert_str_to_float`               | `I64`        | `F64`   |
| `Convert::str_to_bool`            | `Convert_str_to_bool`                | `I64`        | `I64`   |
| `Convert::str_to_array`           | `Convert_str_to_array`               | `I64, I64`   | `I64`   |
| `Convert::str_to_map`             | `Convert_str_to_map`                 | `I64×3`      | `I64`   |
| `Convert::int_to_str`             | `Convert_int_to_str`                 | `I64`        | `I64`   |
| `Convert::int_to_float`           | `Convert_int_to_float`               | `I64`        | `F64`   |
| `Convert::int_to_bool`            | `Convert_int_to_bool`                | `I64`        | `I64`   |
| `Convert::float_to_str`           | `Convert_float_to_str`               | `F64`        | `I64`   |
| `Convert::float_to_int`           | `Convert_float_to_int`               | `F64`        | `I64`   |
| `Convert::float_to_bool`          | `Convert_float_to_bool`              | `F64`        | `I64`   |
| `Convert::bool_to_str`            | `Convert_bool_to_str`                | `I64`        | `I64`   |
| `Convert::bool_to_int`            | `Convert_bool_to_int`                | `I64`        | `I64`   |
| `Convert::bool_to_float`          | `Convert_bool_to_float`              | `I64`        | `F64`   |
| `Convert::array_to_str`           | `Convert_array_to_str`               | `I64, I64`   | `I64`   |
| `Convert::array_to_map`           | `Convert_array_to_map`               | `I64, I64`   | `I64`   |
| `Convert::map_to_str`             | `Convert_map_to_str`                 | `I64×3`      | `I64`   |
| `Convert::map_keys_to_array`      | `Convert_map_keys_to_array`          | `I64`        | `I64`   |
| `Convert::map_values_to_array`    | `Convert_map_values_to_array`        | `I64`        | `I64`   |

---

## Voir aussi

- [examples/builtins/convert.oc](../../examples/builtins/convert.oc) — exemple complet exécutable
- [docs/EBNF.md](../EBNF.md) — grammaire formelle
