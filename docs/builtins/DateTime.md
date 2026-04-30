# DateTime

Classe de la bibliothèque runtime `ocara.DateTime` — manipulation de dates et heures combinées (timestamp Unix).

`DateTime` est une **classe statique** : toutes les méthodes sont appelées directement sur la classe (par exemple `DateTime::now()`).

Les timestamps sont représentés en secondes depuis l'epoch Unix (1er janvier 1970 00:00:00 UTC).

```ocara
import ocara.DateTime
// ou
import ocara.*
```

---

## Méthodes statiques

### `DateTime::now() → int`

Retourne le timestamp Unix actuel (secondes depuis l'epoch).

```ocara
var ts:int = DateTime::now()
IO::write(ts)  // exemple: 1714233600
```

### `DateTime::fromTimestamp(ts:int) → string`

Convertit un timestamp en chaîne au format ISO 8601 : `YYYY-MM-DDTHH:MM:SS`.

```ocara
var ts:int = 1714233600
var dt:string = DateTime::fromTimestamp(ts)
IO::write(dt)  // "2024-04-27T18:00:00"
```

### `DateTime::year(ts:int) → int`

Extrait l'année d'un timestamp.

```ocara
var ts:int = DateTime::now()
var y:int = DateTime::year(ts)
IO::write(y)  // exemple: 2024
```

### `DateTime::month(ts:int) → int`

Extrait le mois d'un timestamp (1-12).

```ocara
var m:int = DateTime::month(ts)  // 1 = janvier, 12 = décembre
```

### `DateTime::day(ts:int) → int`

Extrait le jour du mois d'un timestamp (1-31).

```ocara
var d:int = DateTime::day(ts)
```

### `DateTime::hour(ts:int) → int`

Extrait l'heure d'un timestamp (0-23).

```ocara
var h:int = DateTime::hour(ts)
```

### `DateTime::minute(ts:int) → int`

Extrait les minutes d'un timestamp (0-59).

```ocara
var min:int = DateTime::minute(ts)
```

### `DateTime::second(ts:int) → int`

Extrait les secondes d'un timestamp (0-59).

```ocara
var s:int = DateTime::second(ts)
```

### `DateTime::format(ts:int, fmt:string) → string`

Formate un timestamp selon un pattern personnalisé.

**Patterns supportés** :
- `%Y` — année sur 4 chiffres (ex: `2024`)
- `%m` — mois sur 2 chiffres (ex: `04`)
- `%d` — jour sur 2 chiffres (ex: `27`)
- `%H` — heure sur 2 chiffres (ex: `18`)
- `%M` — minutes sur 2 chiffres (ex: `30`)
- `%S` — secondes sur 2 chiffres (ex: `45`)

```ocara
var ts:int = DateTime::now()
var formatted:string = DateTime::format(ts, "%d/%m/%Y %H:%M:%S")
IO::write(formatted)  // "27/04/2024 18:30:45"
```

### `DateTime::parse(s:string) → int`

Parse une chaîne au format ISO 8601 et retourne le timestamp correspondant.

Formats acceptés :
- `YYYY-MM-DDTHH:MM:SS` (ISO 8601 standard)
- `YYYY-MM-DD HH:MM:SS` (avec espace)

Retourne `0` en cas d'erreur de parsing.

```ocara
var ts:int = DateTime::parse("2024-04-27T18:00:00")
IO::write(ts)  // 1714233600
```

---

## Exemple complet

```ocara
import ocara.DateTime
import ocara.IO

fun main(): void {
    // Timestamp actuel
    var now:int = DateTime::now()
    IO::write("Timestamp actuel: ")
    IO::write(now)
    
    // Conversion en chaîne lisible
    var dt:string = DateTime::fromTimestamp(now)
    IO::write("Date/heure: " + dt)
    
    // Extraction des composants
    IO::write("Année: " + Convert::intToStr(DateTime::year(now)))
    IO::write("Mois: " + Convert::intToStr(DateTime::month(now)))
    IO::write("Jour: " + Convert::intToStr(DateTime::day(now)))
    IO::write("Heure: " + Convert::intToStr(DateTime::hour(now)))
    
    // Formatage personnalisé
    var formatted:string = DateTime::format(now, "Le %d/%m/%Y à %H:%M:%S")
    IO::write(formatted)
    
    // Parsing
    var ts:int = DateTime::parse("2024-12-31T23:59:59")
    IO::write("Timestamp du réveillon: ")
    IO::write(ts)
}
```

---

## Exceptions

### DateTimeException

`DateTime::parse()` lève une **DateTimeException** si le format de la chaîne est invalide.

**Code d'erreur** :

| Code | Signification |
|------|---------------|
| 101  | Invalid datetime format (parsing error) |

**Exemples d'utilisation** :

```ocara
import ocara.DateTime
import ocara.DateTimeException
import ocara.IO

// Exemple 1 : Capture d'exception de parsing
try {
    var ts:int = DateTime::parse("invalid-format")
} on e is DateTimeException {
    IO::writeln(`Erreur: ${e.message}`)
    IO::writeln(`Code: ${e.code}`)
    IO::writeln(`Source: ${e.source}`)
}

// Exemple 2 : Format incomplet
try {
    var ts:int = DateTime::parse("2024-04-27")
} on e is DateTimeException {
    IO::writeln("Format incomplet (manque l'heure)")
}

// Exemple 3 : Valeurs hors plage
try {
    var ts:int = DateTime::parse("2024-13-45T25:70:80")
} on e is DateTimeException {
    IO::writeln(`Valeurs invalides: ${e.message}`)
}

// Exemple 4 : Parsing réussi
try {
    var ts:int = DateTime::parse("2024-04-27T18:30:45")
    IO::writeln(`Success: ${ts}`)
} on e is DateTimeException {
    IO::writeln("Ne devrait pas arriver ici")
}

// Exemple 5 : Catch générique
try {
    var ts:int = DateTime::parse("bad")
} on e {
    IO::writeln(`Exception: ${e.message}`)
    if e.code == 101 {
        IO::writeln("➡️ Code 101 = INVALID_DATETIME_FORMAT")
    }
}
```

**Notes** :

- Seule la méthode `parse()` peut lever une exception
- Toutes les autres méthodes sont **safe** (extractions, formatage)
- Formats acceptés : `YYYY-MM-DDTHH:MM:SS` ou `YYYY-MM-DD HH:MM:SS`
- Les valeurs doivent être dans les plages valides (mois 1-12, jour 1-31, heure 0-23, etc.)

---

## Voir aussi

- [Date](Date.md) — manipulation de dates sans heure
- [Time](Time.md) — manipulation d'heures sans date
- [System](System.md) — autres fonctions système
