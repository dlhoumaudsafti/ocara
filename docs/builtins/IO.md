# `ocara.IO` — Classe de la bibliothèque runtime

> Classe d'entrées/sorties standard.  
> Toutes les méthodes sont **statiques** : elles s'appellent via `IO::<méthode>(args)`.

---

## Import

```ocara
import ocara.IO        // importe uniquement IO
import ocara.*         // importe toutes les classes de la bibliothèque runtime
```

---

## Référence des méthodes

### `IO::write(val)` → `void`

Affiche `val` sur stdout **sans** saut de ligne final.  
Accepte tous les types primitifs (`int`, `float`, `bool`, `string`).

```ocara
IO::write("Ocara IO — ")
IO::write(42)
// affiche: Ocara IO — 42  (sur la même ligne)
```

**Texte multiligne :** les chaînes `"..."` n'acceptent pas les vraies nouvelles lignes.
Utiliser `\n` ou une chaîne template (backticks) :

```ocara
// Avec \n dans une chaîne simple
IO::write("Bonjour David\nComment vas tu ?\n")

// Avec une chaîne template multiligne (recommandé)
IO::write(`Bonjour David
Comment vas tu ?
`)

// Template avec interpolation et multiligne
scoped nom:string = "David"
IO::write(`Bonjour ${nom}
Comment vas tu ?
`)
```

---

### `IO::writeln(val)` → `void`

Affiche `val` sur stdout **suivi d'un saut de ligne**.  
Équivalent à la fonction globale `write()`.

```ocara
IO::writeln("Bonjour Ocara")   // Bonjour Ocara\n
IO::writeln(42)                 // 42\n
IO::writeln(true)               // true\n
```

---

### `IO::read()` → `string`

Lit une ligne depuis stdin et la retourne sans le `\n` final.  
Équivalent à la fonction globale `read()`.

```ocara
IO::write("Votre nom : ")
scoped nom:string = IO::read()
IO::writeln(`Bonjour ${nom} !`)
```

---

### `IO::readln()` → `string`

Alias de `IO::read()`. Comportement identique.

```ocara
scoped ligne:string = IO::readln()
```

---

### `IO::read_int()` → `int`

Lit une ligne depuis stdin et la convertit en `int`.  
Si la saisie n'est pas un entier valide, retourne `0`.

```ocara
IO::write("Entrez un entier : ")
scoped n:int = IO::read_int()
IO::writeln(`Le double : ${n * 2}`)
```

---

### `IO::read_float()` → `float`

Lit une ligne depuis stdin et la convertit en `float`.  
Si la saisie n'est pas un décimal valide, retourne `0.0`.

```ocara
IO::write("Entrez un décimal : ")
scoped f:float = IO::read_float()
IO::writeln(`Valeur : ${f}`)
```

---

### `IO::read_bool()` → `bool`

Lit une ligne depuis stdin et la convertit en `bool`.  
Retourne `true` si la saisie est `"true"` ou `"1"` (insensible à la casse), `false` sinon.

```ocara
IO::write("Continuer ? (true/false) : ")
scoped rep:bool = IO::read_bool()
if rep {
    IO::writeln("Poursuite...")
}
```

---

### `IO::read_array(sep)` → `string[]`

Lit une ligne depuis stdin et la découpe selon le séparateur `sep`.  
Retourne un `string[]`.

| Paramètre | Type     | Description    |
|-----------|----------|----------------|
| `sep`     | `string` | Séparateur     |

```ocara
// Saisie : "rust,ocara,cranelift"
scoped tags:string[] = IO::read_array(",")
// → ["rust", "ocara", "cranelift"]

// Saisie : "10 20 30"
scoped nums:string[] = IO::read_array(" ")
// → ["10", "20", "30"]
```

---

### `IO::read_map(sep, kv)` → `map<string, string>`

Lit une ligne depuis stdin et construit une map clé/valeur.  
- `sep` : séparateur entre les paires  
- `kv`  : séparateur entre la clé et la valeur au sein d'une paire

| Paramètre | Type     | Description                    |
|-----------|----------|--------------------------------|
| `sep`     | `string` | Séparateur entre les paires    |
| `kv`      | `string` | Séparateur clé/valeur          |

```ocara
// Saisie : "lang=fr,theme=dark,limit=50"
scoped cfg:map<string, string> = IO::read_map(",", "=")
Map::get(cfg, "lang")    // → "fr"
Map::get(cfg, "theme")   // → "dark"

// Saisie : "x:10 y:20 z:30"
scoped pts:map<string, string> = IO::read_map(" ", ":")
Map::get(pts, "x")   // → "10"
```

---

## Combinaisons courantes

```ocara
import ocara.IO
import ocara.Array
import ocara.Map

function main(): int {

    // Formulaire simple
    IO::write("Nom   : ")
    scoped nom:string = IO::read()
    IO::write("Âge   : ")
    scoped age:int = IO::read_int()
    IO::writeln(`Bienvenue ${nom}, ${age} ans.`)

    // Lire plusieurs entiers sur une ligne
    IO::writeln("Entrez 3 notes séparées par des espaces :")
    scoped parts:string[] = IO::read_array(" ")
    IO::writeln(`${Array::len(parts)} note(s) reçue(s)`)

    // Configuration inline
    IO::writeln("Paramètres (ex: debug=1,lang=fr) :")
    scoped cfg:map<string, string> = IO::read_map(",", "=")
    if Map::has(cfg, "debug") {
        IO::writeln(`debug activé : ${Map::get(cfg, "debug")}`)
    }

    return 0
}
```

---

## Différence avec `write()` / `read()` globaux

| Fonction globale | Équivalent IO         | Différence                            |
|------------------|-----------------------|---------------------------------------|
| `write(val)`     | `IO::writeln(val)`    | Identique (avec `\n`)                 |
| `read()`         | `IO::read()`          | Identique                             |
| —                | `IO::write(val)`      | Sans `\n` final                       |
| —                | `IO::read_int()`      | Conversion automatique en `int`       |
| —                | `IO::read_float()`    | Conversion automatique en `float`     |
| —                | `IO::read_bool()`     | Conversion automatique en `bool`      |
| —                | `IO::read_array(sep)` | Découpe automatique en `string[]`     |
| —                | `IO::read_map(s, kv)` | Parsing automatique en `map<s,s>`     |

---

## Conventions runtime

| Méthode Ocara       | Symbole runtime C  | Params Cranelift    | Retour  |
|---------------------|--------------------|---------------------|---------|
| `IO::write`         | `IO_write`         | `I64`               | —       |
| `IO::writeln`       | `IO_writeln`       | `I64`               | —       |
| `IO::read`          | `IO_read`          | —                   | `I64`   |
| `IO::readln`        | `IO_readln`        | —                   | `I64`   |
| `IO::read_int`      | `IO_read_int`      | —                   | `I64`   |
| `IO::read_float`    | `IO_read_float`    | —                   | `F64`   |
| `IO::read_bool`     | `IO_read_bool`     | —                   | `I64`   |
| `IO::read_array`    | `IO_read_array`    | `I64`               | `I64`   |
| `IO::read_map`      | `IO_read_map`      | `I64, I64`          | `I64`   |

---

## Voir aussi

- [examples/builtins/io.oc](../../examples/builtins/io.oc) — exemple complet exécutable
- [docs/EBNF.md](../EBNF.md) — grammaire formelle
