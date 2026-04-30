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

### `Convert::strToInt(s)` → `int`

Convertit la chaîne `s` en entier. Retourne `0` si la conversion échoue.

```ocara
Convert::strToInt("42")     // → 42
Convert::strToInt("-7")     // → -7
Convert::strToInt("abc")    // → 0
```

---

### `Convert::strToFloat(s)` → `float`

Convertit la chaîne `s` en décimal. Retourne `0.0` si la conversion échoue.

```ocara
Convert::strToFloat("3.14")   // → 3.14
Convert::strToFloat("42")     // → 42.0
Convert::strToFloat("abc")    // → 0.0
```

---

### `Convert::strToBool(s)` → `bool`

Retourne `true` si `s` vaut `"true"` ou `"1"` (insensible à la casse), `false` sinon.

```ocara
Convert::strToBool("true")    // → true
Convert::strToBool("1")       // → true
Convert::strToBool("false")   // → false
Convert::strToBool("0")       // → false
```

---

### `Convert::strToArray(s, sep)` → `string[]`

Découpe `s` selon le séparateur `sep` et retourne un `string[]`.

```ocara
Convert::strToArray("rust,ocara,web", ",")   // → ["rust", "ocara", "web"]
Convert::strToArray("10 20 30", " ")          // → ["10", "20", "30"]
```

---

### `Convert::strToMap(s, sep, kv)` → `map<string, string>`

Parse `s` en map clé/valeur.  
- `sep` : séparateur entre les paires  
- `kv`  : séparateur clé/valeur au sein d'une paire

```ocara
scoped m:map<string, string> = Convert::strToMap("lang=fr,theme=dark", ",", "=")
// → {"lang": "fr", "theme": "dark"}

scoped m2:map<string, string> = Convert::strToMap("x:10 y:20", " ", ":")
// → {"x": "10", "y": "20"}
```

---

## `int` → *

### `Convert::intToStr(n)` → `string`

```ocara
Convert::intToStr(42)    // → "42"
Convert::intToStr(-7)    // → "-7"
```

### `Convert::intToFloat(n)` → `float`

```ocara
Convert::intToFloat(7)   // → 7.0
```

### `Convert::intToBool(n)` → `bool`

Retourne `false` si `n == 0`, `true` sinon.

```ocara
Convert::intToBool(0)    // → false
Convert::intToBool(1)    // → true
Convert::intToBool(-3)   // → true
```

---

## `float` → *

### `Convert::floatToStr(f)` → `string`

```ocara
Convert::floatToStr(3.14)   // → "3.14"
```

### `Convert::floatToInt(f)` → `int`

Troncature vers zéro (pas d'arrondi).

```ocara
Convert::floatToInt(9.99)    // → 9
Convert::floatToInt(-3.7)    // → -3
```

### `Convert::floatToBool(f)` → `bool`

Retourne `false` si `f == 0.0`, `true` sinon.

```ocara
Convert::floatToBool(0.0)    // → false
Convert::floatToBool(1.5)    // → true
```

---

## `bool` → *

### `Convert::boolToStr(b)` → `string`

```ocara
Convert::boolToStr(true)    // → "true"
Convert::boolToStr(false)   // → "false"
```

### `Convert::boolToInt(b)` → `int`

```ocara
Convert::boolToInt(true)    // → 1
Convert::boolToInt(false)   // → 0
```

### `Convert::boolToFloat(b)` → `float`

```ocara
Convert::boolToFloat(true)    // → 1.0
Convert::boolToFloat(false)   // → 0.0
```

---

## `array` → *

### `Convert::arrayToStr(arr, sep)` → `string`

Joint les éléments du tableau en une chaîne séparée par `sep`.  
Équivalent à `Array::join`.

```ocara
var t:string[] = ["rust", "ocara", "web"]
Convert::arrayToStr(t, ", ")   // → "rust, ocara, web"
Convert::arrayToStr(t, " | ")  // → "rust | ocara | web"
```

### `Convert::arrayToMap(arr, kv)` → `map<string, string>`

Chaque élément du tableau doit être de la forme `"clé<kv>valeur"`.

```ocara
var pairs:string[] = ["lang=fr", "theme=dark", "debug=1"]
scoped m:map<string, string> = Convert::arrayToMap(pairs, "=")
// → {"lang": "fr", "theme": "dark", "debug": "1"}
```

---

## `map` → *

### `Convert::mapToStr(m, sep, kv)` → `string`

Sérialise la map en chaîne. Inverse de `str_to_map`.

```ocara
var m:map<string, string> = {"lang": "fr", "theme": "dark"}
Convert::mapToStr(m, ",", "=")   // → "lang=fr,theme=dark"
```

### `Convert::mapKeysToArray(m)` → `string[]`

Retourne un tableau de toutes les clés. Équivalent à `Map::keys`.

```ocara
scoped cles:string[] = Convert::mapKeysToArray(m)
// → ["lang", "theme"]
```

### `Convert::mapValuesToArray(m)` → `mixed[]`

Retourne un tableau de toutes les valeurs. Équivalent à `Map::values`.

```ocara
scoped vals:mixed[] = Convert::mapValuesToArray(m)
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
    scoped n:int  = Convert::strToInt(s)
    scoped r:string = Convert::intToStr(n / 2)
    write(`Moitié de ${s} = ${r}`)   // 64

    // Parser une config inline
    scoped cfg:map<string, string> = Convert::strToMap("debug=1,lang=fr,limit=50", ",", "=")
    scoped debug:bool = Convert::strToBool(Map::get(cfg, "debug"))
    scoped limit:int  = Convert::strToInt(Map::get(cfg, "limit"))
    write(`debug=${debug}  limit=${limit}`)   // debug=true  limit=50

    // Sérialiser un tableau en CSV puis le reparseur
    var data:string[] = ["alice", "bob", "charlie"]
    scoped csv:string    = Convert::arrayToStr(data, ",")
    scoped back:string[] = Convert::strToArray(csv, ",")
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
| 101 | `INVALID_INT` | `Convert::strToInt()` | Impossible de convertir la chaîne en entier (format invalide) |
| 102 | `INVALID_FLOAT` | `Convert::strToFloat()` | Impossible de convertir la chaîne en flottant (format invalide) |

### Exemples de gestion d'erreurs

#### Conversion string vers int avec gestion d'erreur

```ocara
import ocara.Convert
import ocara.ConvertException
import ocara.IO

function main(): int {
    var input:string = "abc123"
    
    try {
        var num:int = Convert::strToInt(input)
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
            var f:float = Convert::strToFloat(s)
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
        return Convert::strToInt(s)
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
        var num:int = Convert::strToInt(text)
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
        var n:int = Convert::strToInt(val)
        IO::writeln(`${key} (int) = ${n}`)
        return
    } on e is ConvertException {
        // Pas un int, essayer float
    }
    
    try {
        var f:float = Convert::strToFloat(val)
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
- `Convert::strToBool()` ne lève jamais d'exception (retourne false pour valeurs inconnues)
- `Convert::int_to_*()` ne lèvent jamais d'exception (conversions toujours possibles)
- `Convert::float_to_*()` ne lèvent jamais d'exception (troncature pour int, toujours convertible)
- `Convert::bool_to_*()` ne lèvent jamais d'exception (true=1/"true", false=0/"false")
- `Convert::array_to_*()` ne lèvent jamais d'exception
- `Convert::map_to_*()` ne lèvent jamais d'exception

**Seules `strToInt()` et `strToFloat()` peuvent lever des exceptions** car elles nécessitent un format spécifique.

---

## Conventions runtime

| Méthode Ocara                     | Symbole runtime C                    | Params       | Retour  |
|-----------------------------------|--------------------------------------|--------------|---------|
| `Convert::str_to_int`             | `Convert_strToInt`                 | `I64`        | `I64`   |
| `Convert::str_to_float`           | `Convert_strToFloat`               | `I64`        | `F64`   |
| `Convert::str_to_bool`            | `Convert_strToBool`                | `I64`        | `I64`   |
| `Convert::str_to_array`           | `Convert_strToArray`               | `I64, I64`   | `I64`   |
| `Convert::str_to_map`             | `Convert_strToMap`                 | `I64×3`      | `I64`   |
| `Convert::int_to_str`             | `Convert_intToStr`                 | `I64`        | `I64`   |
| `Convert::int_to_float`           | `Convert_intToFloat`               | `I64`        | `F64`   |
| `Convert::int_to_bool`            | `Convert_intToBool`                | `I64`        | `I64`   |
| `Convert::float_to_str`           | `Convert_floatToStr`               | `F64`        | `I64`   |
| `Convert::float_to_int`           | `Convert_floatToInt`               | `F64`        | `I64`   |
| `Convert::float_to_bool`          | `Convert_floatToBool`              | `F64`        | `I64`   |
| `Convert::bool_to_str`            | `Convert_boolToStr`                | `I64`        | `I64`   |
| `Convert::bool_to_int`            | `Convert_boolToInt`                | `I64`        | `I64`   |
| `Convert::bool_to_float`          | `Convert_boolToFloat`              | `I64`        | `F64`   |
| `Convert::array_to_str`           | `Convert_arrayToStr`               | `I64, I64`   | `I64`   |
| `Convert::array_to_map`           | `Convert_arrayToMap`               | `I64, I64`   | `I64`   |
| `Convert::map_to_str`             | `Convert_mapToStr`                 | `I64×3`      | `I64`   |
| `Convert::map_keys_to_array`      | `Convert_mapKeysToArray`          | `I64`        | `I64`   |
| `Convert::map_values_to_array`    | `Convert_mapValuesToArray`        | `I64`        | `I64`   |

---

## Voir aussi

- [examples/builtins/convert.oc](../../examples/builtins/convert.oc) — exemple complet exécutable
- [docs/EBNF.md](../EBNF.md) — grammaire formelle
