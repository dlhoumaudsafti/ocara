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
