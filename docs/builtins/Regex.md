# `ocara.Regex` — Classe builtin

> Classe de traitement des expressions régulières.  
> Toutes les méthodes sont **statiques** : elles s'appellent via `Regex::<méthode>(args)`.  
> Les patterns suivent la syntaxe **POSIX ERE** (Extended Regular Expressions).

---

## Import

```ocara
import ocara.Regex        // importe uniquement Regex
import ocara.*            // importe toutes les classes builtins
```

---

## Référence des méthodes

### `Regex::test(pattern, s)` → `bool`

Retourne `true` si `s` contient au moins une correspondance avec `pattern`.  
Pour un match complet, ancrer avec `^` et `$`.

```ocara
Regex::test("[0-9]+",    "abc123")   // → true
Regex::test("^[0-9]+$", "abc123")   // → false  (lettres présentes)
Regex::test("^[a-z]+$", "hello")    // → true
```

---

### `Regex::find(pattern, s)` → `string`

Retourne la **première** sous-chaîne correspondant à `pattern`.  
Retourne une chaîne vide si aucune correspondance.

```ocara
Regex::find("[0-9]+",      "prix: 42 euros")     // → "42"
Regex::find("[A-Z][a-z]+", "Bonjour Alice")       // → "Bonjour"
Regex::find("[0-9]+",      "aucun chiffre ici")   // → ""
```

---

### `Regex::find_all(pattern, s)` → `string[]`

Retourne **toutes** les sous-chaînes correspondant à `pattern` sous forme de tableau.

```ocara
scoped nums:string[] = Regex::find_all("[0-9]+", "a1 b22 c333")
// → ["1", "22", "333"]

scoped mots:string[] = Regex::find_all("[a-z]+", "hello world foo")
// → ["hello", "world", "foo"]
```

---

### `Regex::replace(pattern, s, repl)` → `string`

Remplace la **première** occurrence de `pattern` dans `s` par `repl`.

| Paramètre | Type     | Description              |
|-----------|----------|--------------------------|
| `pattern` | `string` | Expression régulière     |
| `s`       | `string` | Chaîne source            |
| `repl`    | `string` | Chaîne de remplacement   |

```ocara
Regex::replace("[0-9]+", "ref-123-abc-456", "NUM")
// → "ref-NUM-abc-456"
```

---

### `Regex::replace_all(pattern, s, repl)` → `string`

Remplace **toutes** les occurrences de `pattern` dans `s` par `repl`.

```ocara
Regex::replace_all("[0-9]+", "ref-123-abc-456", "NUM")
// → "ref-NUM-abc-NUM"

Regex::replace_all("\\s+", "  espaces   multiples  ", " ")
// → " espaces multiples "
```

---

### `Regex::split(pattern, s)` → `string[]`

Découpe `s` en utilisant `pattern` comme séparateur.

```ocara
Regex::split("[,;|]+", "a,b;;c|d")
// → ["a", "b", "c", "d"]

Regex::split("\\s+", "un  deux   trois")
// → ["un", "deux", "trois"]
```

---

### `Regex::count(pattern, s)` → `int`

Retourne le nombre de correspondances non-chevauchantes de `pattern` dans `s`.

```ocara
Regex::count("[aeiou]", "bonjour monde")              // → 4
Regex::count("\\d+",   "2 chats et 3 chiens et 1")   // → 3
Regex::count("[0-9]+", "aucun chiffre")               // → 0
```

---

### `Regex::extract(pattern, s, group)` → `string`

Retourne le contenu du groupe de capture numéroté `group` (1-indexé) de la première correspondance.  
Retourne une chaîne vide si le groupe n'existe pas ou si aucune correspondance.

| Paramètre | Type     | Description                       |
|-----------|----------|-----------------------------------|
| `pattern` | `string` | Expression régulière avec groupes |
| `s`       | `string` | Chaîne source                     |
| `group`   | `int`    | Numéro de groupe (commence à 1)   |

```ocara
scoped date:string = "2026-04-25"
Regex::extract("(\\d{4})-(\\d{2})-(\\d{2})", date, 1)   // → "2026"
Regex::extract("(\\d{4})-(\\d{2})-(\\d{2})", date, 2)   // → "04"
Regex::extract("(\\d{4})-(\\d{2})-(\\d{2})", date, 3)   // → "25"

Regex::extract("([^@]+)@(.+)", "user@example.com", 1)   // → "user"
Regex::extract("([^@]+)@(.+)", "user@example.com", 2)   // → "example.com"
```

---

## Patterns courants

| Usage                     | Pattern                                         |
|---------------------------|-------------------------------------------------|
| Entier                    | `[0-9]+` ou `\\d+`                             |
| Décimal                   | `[0-9]+\\.[0-9]+`                              |
| Mot                       | `[a-zA-Z]+`                                     |
| Identifiant               | `[a-zA-Z_][a-zA-Z0-9_]*`                       |
| Adresse e-mail            | `[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}` |
| URL HTTP(S)               | `https?://[^\\s]+`                              |
| Date YYYY-MM-DD           | `(\\d{4})-(\\d{2})-(\\d{2})`                  |
| Code postal français      | `[0-9]{5}`                                      |
| Espaces multiples         | `\\s+`                                          |
| Ligne vide                | `^\\s*$`                                        |

---

## Exemples combinés

```ocara
import ocara.Regex

function main(): int {

    // Validation d'un e-mail
    var email:string = "contact@ocara-lang.org"
    if Regex::test("^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$", email) {
        write(`${email} est valide`)
    }

    // Extraire toutes les URLs d'un texte
    var texte:string = "voir https://ocara.dev et https://github.com/ocara"
    scoped urls:string[] = Regex::find_all("https?://[^\\s]+", texte)
    scoped nb:int = Regex::count("https?://[^\\s]+", texte)
    write(`${nb} URL(s) trouvée(s)`)

    // Normaliser des séparateurs
    scoped csv:string = Regex::replace_all("[,;\\t]+", "a,b;c\td", ",")
    write(csv)   // a,b,c,d

    // Extraire année/mois/jour d'une date
    scoped d:string = "Livraison le 2026-04-25"
    scoped an:string  = Regex::extract("(\\d{4})-(\\d{2})-(\\d{2})", d, 1)
    scoped mo:string  = Regex::extract("(\\d{4})-(\\d{2})-(\\d{2})", d, 2)
    scoped jo:string  = Regex::extract("(\\d{4})-(\\d{2})-(\\d{2})", d, 3)
    write(`Année=${an} Mois=${mo} Jour=${jo}`)

    return 0
}
```

---

## Gestion d'erreurs

Toutes les méthodes Regex peuvent lever une `RegexException` en cas d'erreur.

### Codes d'erreur RegexException

| Code | Nom | Opération | Description |
|------|------|-----------|-------------|
| 101 | `INVALID_PATTERN` | Toutes les méthodes | Pattern d'expression régulière invalide (erreur de syntaxe) |

### Exemples de gestion d'erreurs

#### Gestion de pattern invalide

```ocara
import ocara.Regex
import ocara.RegexException
import ocara.IO

function main(): int {
    var text:string = "Hello World 123"
    var bad_pattern:string = "[0-9"  // Pattern invalide (crochet non fermé)
    
    try {
        var result:bool = Regex::test(bad_pattern, text)
        IO::writeln(`Result: ${result}`)
    } on e is RegexException {
        IO::writeln(`Regex error: ${e.message}`)
        IO::writeln(`Code: ${e.code}`)
        if e.code == 101 {
            IO::writeln("Invalid regex pattern syntax")
        }
    }
    
    return 0
}
```

#### Fonction safe_regex_test avec valeur par défaut

```ocara
import ocara.Regex
import ocara.RegexException
import ocara.IO

function safe_regex_test(pattern:string, text:string): bool {
    try {
        return Regex::test(pattern, text)
    } on e is RegexException {
        IO::writeln(`Invalid pattern: ${pattern}`)
        return false
    }
}

function main(): int {
    var text:string = "Contact: email@example.com"
    
    // Pattern valide
    var valid:bool = safe_regex_test("\\w+@\\w+\\.\\w+", text)
    IO::writeln(`Valid email pattern: ${valid}`)
    
    // Pattern invalide
    var invalid:bool = safe_regex_test("(unclosed", text)
    IO::writeln(`Invalid pattern result: ${invalid}`)
    
    return 0
}
```

#### Validation de pattern regex

```ocara
import ocara.Regex
import ocara.RegexException
import ocara.IO

function validate_regex_pattern(pattern:string): bool {
    try {
        // Tester avec une chaîne vide pour valider la syntaxe
        Regex::test(pattern, "")
        return true
    } on e is RegexException {
        return false
    }
}

function main(): int {
    var patterns:string[] = [
        "\\d+",           // Valide
        "[a-z]+",         // Valide
        "(?P<name>\\w+)", // Valide
        "[0-9",           // Invalide (crochet non fermé)
        "(?P<",           // Invalide (groupe nommé incomplet)
        "*",              // Invalide (répétition sans cible)
    ]
    
    scoped i:int = 0
    scoped len:int = Array::len(patterns)
    while i < len {
        var p:string = patterns.get(i)
        var valid:bool = validate_regex_pattern(p)
        if valid {
            IO::writeln(`✓ '${p}' is valid`)
        } else {
            IO::writeln(`✗ '${p}' is invalid`)
        }
        i = i + 1
    }
    
    return 0
}
```

#### Catch générique

```ocara
import ocara.Regex
import ocara.IO

function main(): int {
    var pattern:string = "\\d{2,1}"  // Invalide: min > max
    var text:string = "123"
    
    try {
        var matches:string[] = Regex::find_all(pattern, text)
        IO::writeln(`Found ${Array::len(matches)} matches`)
    } on e {
        // Capture toute exception
        IO::writeln(`Exception: ${e.message}`)
        IO::writeln(`Source: ${e.source}`)
        IO::writeln(`Code: ${e.code}`)
    }
    
    return 0
}
```

#### Multiple opérations avec pattern dynamique

```ocara
import ocara.Regex
import ocara.RegexException
import ocara.IO

function search_with_pattern(pattern:string, texts:string[]): void {
    IO::writeln(`Pattern: ${pattern}`)
    
    try {
        scoped i:int = 0
        scoped len:int = Array::len(texts)
        while i < len {
            var text:string = texts.get(i)
            var found:bool = Regex::test(pattern, text)
            if found {
                var match:string = Regex::find(pattern, text)
                IO::writeln(`  ✓ Found in '${text}': '${match}'`)
            } else {
                IO::writeln(`  - Not found in '${text}'`)
            }
            i = i + 1
        }
    } on e is RegexException {
        IO::writeln(`  ✗ Invalid pattern: ${e.message}`)
    }
    
    IO::writeln("")
}

function main(): int {
    var texts:string[] = ["Hello 123", "World 456", "Test ABC"]
    
    search_with_pattern("\\d+", texts)        // Valide
    search_with_pattern("[A-Z]+", texts)      // Valide
    search_with_pattern("(broken", texts)     // Invalide
    
    return 0
}
```

#### Remplacement avec pattern invalide

```ocara
import ocara.Regex
import ocara.RegexException
import ocara.IO

function safe_replace(pattern:string, text:string, replacement:string): string {
    try {
        return Regex::replace_all(pattern, text, replacement)
    } on e is RegexException {
        IO::writeln(`Cannot replace with invalid pattern '${pattern}'`)
        return text  // Retourne le texte original
    }
}

function main(): int {
    var text:string = "Hello 123 World 456"
    
    // Remplacement valide
    var result1:string = safe_replace("\\d+", text, "XXX")
    IO::writeln(`Result 1: ${result1}`)
    
    // Pattern invalide
    var result2:string = safe_replace("[0-9", text, "YYY")
    IO::writeln(`Result 2: ${result2}`)
    
    return 0
}
```

### Format des messages d'exception

Les messages d'exception sont en anglais et incluent le pattern problématique ainsi que l'erreur de syntaxe :
- `Invalid regex pattern: '[0-9' (regex parse error: unclosed character class)`
- `Invalid regex pattern: '*' (regex parse error: repetition operator missing expression)`
- `Invalid regex pattern: '(?P<' (regex parse error: unclosed group name)`

**Notes importantes :**
- **Toutes les méthodes Regex peuvent lever une exception** si le pattern est invalide
- L'exception est levée lors de la compilation du pattern, pas lors de l'exécution du match
- Il est recommandé de valider les patterns regex provenant de l'utilisateur avec try/on
- Les patterns regex suivent la syntaxe de la crate Rust `regex`
- Pour tester si un pattern est valide sans exception, utilisez try/on avec une opération quelconque

**Liste des méthodes qui peuvent lever RegexException :**
- `Regex::test()` - Test si le pattern match
- `Regex::find()` - Trouve la première correspondance
- `Regex::find_all()` - Trouve toutes les correspondances
- `Regex::replace()` - Remplace la première occurrence
- `Regex::replace_all()` - Remplace toutes les occurrences
- `Regex::split()` - Découpe selon le pattern
- `Regex::count()` - Compte les correspondances
- `Regex::extract()` - Extrait un groupe de capture

---

## Conventions runtime

| Méthode Ocara           | Symbole runtime C     | Params Cranelift        | Retour  |
|-------------------------|-----------------------|-------------------------|---------|
| `Regex::test`           | `Regex_test`          | `I64, I64`              | `I64`   |
| `Regex::find`           | `Regex_find`          | `I64, I64`              | `I64`   |
| `Regex::find_all`       | `Regex_find_all`      | `I64, I64`              | `I64`   |
| `Regex::replace`        | `Regex_replace`       | `I64, I64, I64`         | `I64`   |
| `Regex::replace_all`    | `Regex_replace_all`   | `I64, I64, I64`         | `I64`   |
| `Regex::split`          | `Regex_split`         | `I64, I64`              | `I64`   |
| `Regex::count`          | `Regex_count`         | `I64, I64`              | `I64`   |
| `Regex::extract`        | `Regex_extract`       | `I64, I64, I64`         | `I64`   |

Les chaînes sont passées comme pointeurs `I64` (adresses mémoire dans le heap géré par le runtime).  
`Regex_test` retourne `0` (false) ou `1` (true) encodé en `I64`.

---

## Voir aussi

- [examples/builtins/regex.oc](../../examples/builtins/regex.oc) — exemple complet exécutable
- [docs/EBNF.md](../EBNF.md) — grammaire formelle
