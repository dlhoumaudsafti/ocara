# Time

Classe de la bibliothèque runtime `ocara.Time` — manipulation d'heures (sans date).

`Time` est une **classe statique** : toutes les méthodes sont appelées directement sur la classe (par exemple `Time::now()`).

Les heures sont représentées au format `HH:MM:SS` (24 heures).

```ocara
import ocara.Time
// ou
import ocara.*
```

---

## Méthodes statiques

### `Time::now() → string`

Retourne l'heure actuelle au format `HH:MM:SS`.

```ocara
var t:string = Time::now()
IO::write(t)  // "18:30:45"
```

### `Time::from_timestamp(ts:int) → string`

Extrait l'heure d'un timestamp Unix au format `HH:MM:SS`.

```ocara
var ts:int = 1714233600
var time:string = Time::from_timestamp(ts)
IO::write(time)  // "18:00:00"
```

### `Time::hour(time:string) → int`

Extrait l'heure d'un time `HH:MM:SS` (0-23).

```ocara
var h:int = Time::hour("18:30:45")
IO::write(h)  // 18
```

### `Time::minute(time:string) → int`

Extrait les minutes d'un time (0-59).

```ocara
var m:int = Time::minute("18:30:45")
IO::write(m)  // 30
```

### `Time::second(time:string) → int`

Extrait les secondes d'un time (0-59).

```ocara
var s:int = Time::second("18:30:45")
IO::write(s)  // 45
```

### `Time::from_seconds(seconds:int) → string`

Convertit un nombre de secondes depuis minuit en format `HH:MM:SS`.

Les valeurs >= 86400 (24h) sont automatiquement ramenées dans la journée (modulo 86400).

```ocara
var t1:string = Time::from_seconds(3661)
IO::write(t1)  // "01:01:01" (1h + 1min + 1s)

var t2:string = Time::from_seconds(90000)
IO::write(t2)  // "01:00:00" (90000 % 86400 = 3600)
```

### `Time::to_seconds(time:string) → int`

Convertit un time `HH:MM:SS` en nombre de secondes depuis minuit.

```ocara
var s:int = Time::to_seconds("18:30:45")
IO::write(s)  // 66645 (18*3600 + 30*60 + 45)
```

### `Time::add_seconds(time:string, s:int) → string`

Ajoute un nombre de secondes (positif ou négatif) à un time.

Le résultat reste dans l'intervalle 00:00:00 - 23:59:59 (modulo 24h).

```ocara
var t1:string = Time::add_seconds("18:30:00", 3600)
IO::write(t1)  // "19:30:00" (+1h)

var t2:string = Time::add_seconds("23:30:00", 3600)
IO::write(t2)  // "00:30:00" (déborde sur le jour suivant)

var t3:string = Time::add_seconds("02:00:00", -7200)
IO::write(t3)  // "00:00:00" (-2h)
```

### `Time::diff_seconds(t1:string, t2:string) → int`

Calcule la différence en secondes entre deux times (t1 - t2).

```ocara
var diff:int = Time::diff_seconds("19:30:00", "18:30:00")
IO::write(diff)  // 3600 (1h)

diff = Time::diff_seconds("18:30:00", "19:30:00")
IO::write(diff)  // -3600
```

---

## Exemple complet

```ocara
import ocara.Time
import ocara.IO
import ocara.Convert

fun main(): void {
    // Heure actuelle
    var now:string = Time::now()
    IO::write("Heure actuelle: " + now)
    
    // Extraction des composants
    var hour:int = Time::hour(now)
    var minute:int = Time::minute(now)
    var second:int = Time::second(now)
    IO::write("Heure: " + Convert::int_to_str(hour))
    IO::write("Minutes: " + Convert::int_to_str(minute))
    IO::write("Secondes: " + Convert::int_to_str(second))
    
    // Conversion en secondes
    var total_seconds:int = Time::to_seconds(now)
    IO::write("Secondes depuis minuit: " + Convert::int_to_str(total_seconds))
    
    // Reconversion
    var back:string = Time::from_seconds(total_seconds)
    IO::write("Reconverti: " + back)
    
    // Calculs sur les heures
    var later:string = Time::add_seconds(now, 7200)
    IO::write("Dans 2 heures: " + later)
    
    var diff:int = Time::diff_seconds(later, now)
    IO::write("Différence: " + Convert::int_to_str(diff) + " secondes")
    
    // Gestion du débordement
    var midnight:string = Time::add_seconds("23:00:00", 7200)
    IO::write("23:00:00 + 2h = " + midnight)  // "01:00:00"
}
```

---

## Exceptions

### TimeException

Plusieurs méthodes lèvent une **TimeException** si le format du temps est invalide.

**Code d'erreur** :

| Code | Signification |
|------|---------------|
| 101  | Invalid time format (parsing error) |

**Méthodes concernées** :

- `Time::hour(time)` — format invalide ou heure hors plage (0-23)
- `Time::minute(time)` — format invalide ou minutes hors plage (0-59)
- `Time::second(time)` — format invalide ou secondes hors plage (0-59)
- `Time::to_seconds(time)` — time invalide (appelle les fonctions ci-dessus)
- `Time::add_seconds(time, s)` — time de départ invalide
- `Time::diff_seconds(t1, t2)` — l'un des deux times invalide

**Exemples d'utilisation** :

```ocara
import ocara.Time
import ocara.TimeException
import ocara.IO

// Exemple 1 : Format incomplet
try {
    var h:int = Time::hour("18:30")
} on e is TimeException {
    IO::writeln(`Erreur: ${e.message}`)
    IO::writeln(`Code: ${e.code}`)
}

// Exemple 2 : Heure invalide
try {
    var h:int = Time::hour("25:30:00")
} on e is TimeException {
    IO::writeln("Heure hors plage (doit être 0-23)")
}

// Exemple 3 : Minutes invalides
try {
    var m:int = Time::minute("18:70:00")
} on e is TimeException {
    IO::writeln("Minutes hors plage (doivent être 0-59)")
}

// Exemple 4 : Extraction réussie
try {
    var h:int = Time::hour("18:30:45")
    var m:int = Time::minute("18:30:45")
    var s:int = Time::second("18:30:45")
    IO::writeln(`Time valide: ${h}:${m}:${s}`)
} on e is TimeException {
    IO::writeln("Ne devrait pas arriver ici")
}

// Exemple 5 : Catch générique
try {
    var seconds:int = Time::to_seconds("invalid")
} on e {
    IO::writeln(`Exception: ${e.message}`)
    if e.code == 101 {
        IO::writeln("➡️ Code 101 = INVALID_TIME_FORMAT")
    }
}
```

**Notes** :

- Les méthodes **safe** (qui ne lèvent jamais d'exception) :
  - `Time::now()` — toujours valide
  - `Time::from_timestamp(ts)` — toujours valide
  - `Time::from_seconds(seconds)` — toujours valide (modulo 86400)
- Format requis : `HH:MM:SS` avec séparateur `:`
- Les valeurs doivent être dans les plages valides (heure 0-23, minutes 0-59, secondes 0-59)

---

## Remarques

- Les calculs sur les times ne gèrent **pas** les changements de jour. Si vous avez besoin de calculer des durées sur plusieurs jours, utilisez [DateTime](DateTime.md) avec des timestamps.
- Le format est toujours sur 2 chiffres : `01:05:09` et non `1:5:9`.
- Les valeurs hors plage (ex: `25:00:00`) ne sont pas validées par les méthodes `hour()`, `minute()`, `second()`.

---

## Voir aussi

- [DateTime](DateTime.md) — manipulation de dates et heures combinées
- [Date](Date.md) — manipulation de dates sans heure
