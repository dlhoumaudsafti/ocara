# Date

Classe de la bibliothèque runtime `ocara.Date` — manipulation de dates (sans heure).

`Date` est une **classe statique** : toutes les méthodes sont appelées directement sur la classe (par exemple `Date::today()`).

Les dates sont représentées au format `YYYY-MM-DD` (ISO 8601).

```ocara
import ocara.Date
// ou
import ocara.*
```

---

## Méthodes statiques

### `Date::today() → string`

Retourne la date actuelle au format `YYYY-MM-DD`.

```ocara
var d:string = Date::today()
IO::write(d)  // "2024-04-27"
```

### `Date::fromTimestamp(ts:int) → string`

Convertit un timestamp Unix en date au format `YYYY-MM-DD`.

```ocara
var ts:int = 1714233600
var date:string = Date::fromTimestamp(ts)
IO::write(date)  // "2024-04-27"
```

### `Date::year(date:string) → int`

Extrait l'année d'une date `YYYY-MM-DD`.

```ocara
var y:int = Date::year("2024-04-27")
IO::write(y)  // 2024
```

### `Date::month(date:string) → int`

Extrait le mois d'une date (1-12).

```ocara
var m:int = Date::month("2024-04-27")  // 4 (avril)
```

### `Date::day(date:string) → int`

Extrait le jour du mois d'une date (1-31).

```ocara
var d:int = Date::day("2024-04-27")  // 27
```

### `Date::dayOfWeek(date:string) → int`

Retourne le jour de la semaine (0 = lundi, 6 = dimanche).

```ocara
var dow:int = Date::dayOfWeek("2024-04-27")
if dow == 0 {
    IO::write("Lundi")
} else if dow == 6 {
    IO::write("Dimanche")
}
```

**Correspondance** :
- `0` = lundi
- `1` = mardi
- `2` = mercredi
- `3` = jeudi
- `4` = vendredi
- `5` = samedi
- `6` = dimanche

### `Date::isLeapYear(year:int) → bool`

Retourne `true` si l'année est bissextile, `false` sinon.

Une année est bissextile si :
- Elle est divisible par 4 **et** non divisible par 100, **ou**
- Elle est divisible par 400

```ocara
if Date::isLeapYear(2024) {
    IO::write("2024 est bissextile")  // true
}
if Date::isLeapYear(2100) {
    IO::write("2100 est bissextile")  // false (divisible par 100 mais pas par 400)
}
```

### `Date::daysInMonth(year:int, month:int) → int`

Retourne le nombre de jours dans un mois donné (prend en compte les années bissextiles).

```ocara
var days:int = Date::daysInMonth(2024, 2)
IO::write(days)  // 29 (février 2024, année bissextile)

days = Date::daysInMonth(2023, 2)
IO::write(days)  // 28 (février 2023, année normale)
```

### `Date::addDays(date:string, days:int) → string`

Ajoute un nombre de jours (positif ou négatif) à une date.

```ocara
var d1:string = Date::addDays("2024-04-27", 10)
IO::write(d1)  // "2024-05-07"

var d2:string = Date::addDays("2024-04-27", -10)
IO::write(d2)  // "2024-04-17"
```

### `Date::diffDays(date1:string, date2:string) → int`

Calcule la différence en jours entre deux dates (date1 - date2).

```ocara
var diff:int = Date::diffDays("2024-05-07", "2024-04-27")
IO::write(diff)  // 10

diff = Date::diffDays("2024-04-27", "2024-05-07")
IO::write(diff)  // -10
```

---

## Exemple complet

```ocara
import ocara.Date
import ocara.IO
import ocara.Convert

fun main(): void {
    // Date actuelle
    var today:string = Date::today()
    IO::write("Aujourd'hui: " + today)
    
    // Extraction des composants
    var year:int = Date::year(today)
    var month:int = Date::month(today)
    var day:int = Date::day(today)
    IO::write("Année: " + Convert::intToStr(year))
    IO::write("Mois: " + Convert::intToStr(month))
    IO::write("Jour: " + Convert::intToStr(day))
    
    // Jour de la semaine
    var dow:int = Date::dayOfWeek(today)
    IO::write("Jour de la semaine: " + Convert::intToStr(dow))
    
    // Année bissextile
    if Date::isLeapYear(year) {
        IO::write("Année bissextile")
    } else {
        IO::write("Année normale")
    }
    
    // Calculs sur les dates
    var future:string = Date::addDays(today, 100)
    IO::write("Dans 100 jours: " + future)
    
    var diff:int = Date::diffDays(future, today)
    IO::write("Différence: " + Convert::intToStr(diff) + " jours")
}
```

---

## Exceptions

### DateException

Plusieurs méthodes lèvent une **DateException** si le format de la date est invalide.

**Code d'erreur** :

| Code | Signification |
|------|---------------|
| 101  | Invalid date format (parsing error) |

**Méthodes concernées** :

- `Date::year(date)` — format invalide ou année non parsable
- `Date::month(date)` — format invalide ou mois hors plage (1-12)
- `Date::day(date)` — format invalide ou jour hors plage (1-31)
- `Date::dayOfWeek(date)` — date invalide (appelle les fonctions ci-dessus)
- `Date::addDays(date, days)` — date de départ invalide
- `Date::diffDays(date1, date2)` — l'une des deux dates invalide

**Exemples d'utilisation** :

```ocara
import ocara.Date
import ocara.DateException
import ocara.IO

// Exemple 1 : Format incomplet
try {
    var y:int = Date::year("2024")
} on e is DateException {
    IO::writeln(`Erreur: ${e.message}`)
    IO::writeln(`Code: ${e.code}`)
}

// Exemple 2 : Mois invalide
try {
    var m:int = Date::month("2024-13-01")
} on e is DateException {
    IO::writeln("Mois hors plage (doit être 1-12)")
}

// Exemple 3 : Format avec séparateur incorrect
try {
    var d:int = Date::day("2024/04/27")
} on e is DateException {
    IO::writeln("Format attendu: YYYY-MM-DD")
}

// Exemple 4 : Extraction réussie
try {
    var y:int = Date::year("2024-04-27")
    var m:int = Date::month("2024-04-27")
    var d:int = Date::day("2024-04-27")
    IO::writeln(`Date valide: ${y}/${m}/${d}`)
} on e is DateException {
    IO::writeln("Ne devrait pas arriver ici")
}

// Exemple 5 : Catch générique
try {
    var dow:int = Date::dayOfWeek("invalid")
} on e {
    IO::writeln(`Exception: ${e.message}`)
    if e.code == 101 {
        IO::writeln("➡️ Code 101 = INVALID_DATE_FORMAT")
    }
}
```

**Notes** :

- Les méthodes **safe** (qui ne lèvent jamais d'exception) :
  - `Date::today()` — toujours valide
  - `Date::fromTimestamp(ts)` — toujours valide
  - `Date::isLeapYear(year)` — toujours valide
  - `Date::daysInMonth(year, month)` — retourne 0 si mois invalide
- Format requis : `YYYY-MM-DD` avec séparateur `-`
- Les valeurs doivent être dans les plages valides (mois 1-12, jour 1-31)

---

## Voir aussi

- [DateTime](DateTime.md) — manipulation de dates et heures combinées
- [Time](Time.md) — manipulation d'heures sans date
