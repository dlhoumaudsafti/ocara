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

### `DateTime::from_timestamp(ts:int) → string`

Convertit un timestamp en chaîne au format ISO 8601 : `YYYY-MM-DDTHH:MM:SS`.

```ocara
var ts:int = 1714233600
var dt:string = DateTime::from_timestamp(ts)
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
    var dt:string = DateTime::from_timestamp(now)
    IO::write("Date/heure: " + dt)
    
    // Extraction des composants
    IO::write("Année: " + Convert::int_to_str(DateTime::year(now)))
    IO::write("Mois: " + Convert::int_to_str(DateTime::month(now)))
    IO::write("Jour: " + Convert::int_to_str(DateTime::day(now)))
    IO::write("Heure: " + Convert::int_to_str(DateTime::hour(now)))
    
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

## Voir aussi

- [Date](Date.md) — manipulation de dates sans heure
- [Time](Time.md) — manipulation d'heures sans date
- [System](System.md) — autres fonctions système
