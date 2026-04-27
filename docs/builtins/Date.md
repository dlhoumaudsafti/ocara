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

### `Date::from_timestamp(ts:int) → string`

Convertit un timestamp Unix en date au format `YYYY-MM-DD`.

```ocara
var ts:int = 1714233600
var date:string = Date::from_timestamp(ts)
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

### `Date::day_of_week(date:string) → int`

Retourne le jour de la semaine (0 = lundi, 6 = dimanche).

```ocara
var dow:int = Date::day_of_week("2024-04-27")
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

### `Date::is_leap_year(year:int) → bool`

Retourne `true` si l'année est bissextile, `false` sinon.

Une année est bissextile si :
- Elle est divisible par 4 **et** non divisible par 100, **ou**
- Elle est divisible par 400

```ocara
if Date::is_leap_year(2024) {
    IO::write("2024 est bissextile")  // true
}
if Date::is_leap_year(2100) {
    IO::write("2100 est bissextile")  // false (divisible par 100 mais pas par 400)
}
```

### `Date::days_in_month(year:int, month:int) → int`

Retourne le nombre de jours dans un mois donné (prend en compte les années bissextiles).

```ocara
var days:int = Date::days_in_month(2024, 2)
IO::write(days)  // 29 (février 2024, année bissextile)

days = Date::days_in_month(2023, 2)
IO::write(days)  // 28 (février 2023, année normale)
```

### `Date::add_days(date:string, days:int) → string`

Ajoute un nombre de jours (positif ou négatif) à une date.

```ocara
var d1:string = Date::add_days("2024-04-27", 10)
IO::write(d1)  // "2024-05-07"

var d2:string = Date::add_days("2024-04-27", -10)
IO::write(d2)  // "2024-04-17"
```

### `Date::diff_days(date1:string, date2:string) → int`

Calcule la différence en jours entre deux dates (date1 - date2).

```ocara
var diff:int = Date::diff_days("2024-05-07", "2024-04-27")
IO::write(diff)  // 10

diff = Date::diff_days("2024-04-27", "2024-05-07")
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
    IO::write("Année: " + Convert::int_to_str(year))
    IO::write("Mois: " + Convert::int_to_str(month))
    IO::write("Jour: " + Convert::int_to_str(day))
    
    // Jour de la semaine
    var dow:int = Date::day_of_week(today)
    IO::write("Jour de la semaine: " + Convert::int_to_str(dow))
    
    // Année bissextile
    if Date::is_leap_year(year) {
        IO::write("Année bissextile")
    } else {
        IO::write("Année normale")
    }
    
    // Calculs sur les dates
    var future:string = Date::add_days(today, 100)
    IO::write("Dans 100 jours: " + future)
    
    var diff:int = Date::diff_days(future, today)
    IO::write("Différence: " + Convert::int_to_str(diff) + " jours")
}
```

---

## Voir aussi

- [DateTime](DateTime.md) — manipulation de dates et heures combinées
- [Time](Time.md) — manipulation d'heures sans date
