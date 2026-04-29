# `ocara.String` — Classe de la bibliothèque runtime

> Classe de manipulation de chaînes de caractères.  
> Toutes les méthodes sont **statiques** : elles s'appellent via `String::<méthode>(args)`.

---

## Import

```ocara
import ocara.String        // importe uniquement String
import ocara.*             // importe toutes les classes builtins
```

Après l'import, la classe est accessible sous le nom `String` (ou un alias de votre choix) :

```ocara
import ocara.String as Str

scoped n:int = Str::len("bonjour")
```

---

## Référence des méthodes

### `String::len(s)` → `int`

Retourne le nombre de caractères de la chaîne `s`.

```ocara
String::len("Ocara")   // → 5
String::len("")        // → 0
```

---

### `String::upper(s)` → `string`

Convertit tous les caractères de `s` en majuscules.

```ocara
String::upper("ocara")   // → "OCARA"
String::upper("Bonjour") // → "BONJOUR"
```

---

### `String::lower(s)` → `string`

Convertit tous les caractères de `s` en minuscules.

```ocara
String::lower("OCARA")   // → "ocara"
String::lower("Bonjour") // → "bonjour"
```

---

### `String::capitalize(s)` → `string`

Met la première lettre de `s` en majuscule, laisse le reste intact.

```ocara
String::capitalize("bonjour monde")   // → "Bonjour monde"
String::capitalize("ocara")           // → "Ocara"
```

---

### `String::trim(s)` → `string`

Supprime les espaces (et tabulations) en début et en fin de chaîne.

```ocara
String::trim("  bonjour  ")   // → "bonjour"
String::trim("\t texte \n")   // → "texte"
```

---

### `String::replace(s, from, to)` → `string`

Remplace **toutes** les occurrences de la sous-chaîne `from` par `to` dans `s`.

| Paramètre | Type     | Description              |
|-----------|----------|--------------------------|
| `s`       | `string` | Chaîne source            |
| `from`    | `string` | Sous-chaîne à remplacer  |
| `to`      | `string` | Chaîne de remplacement   |

```ocara
String::replace("chat noir chat blanc", "chat", "chien")
// → "chien noir chien blanc"

String::replace("a-b-c", "-", ".")
// → "a.b.c"
```

---

### `String::split(s, sep)` → `string[]`

Découpe `s` en un tableau de sous-chaînes, en utilisant `sep` comme séparateur.  
Voir aussi [`String::explode`](#stringexplodes-sep--string) — fonctionnellement identique.

| Paramètre | Type     | Description   |
|-----------|----------|---------------|
| `s`       | `string` | Chaîne source |
| `sep`     | `string` | Séparateur    |

```ocara
scoped mots:string[] = String::split("alice,bob,charlie", ",")
// → ["alice", "bob", "charlie"]

scoped lignes:string[] = String::split("a\nb\nc", "\n")
// → ["a", "b", "c"]
```

---

### `String::explode(s, sep)` → `string[]`

Alias de [`String::split`](#stringsplits-sep--string). Comportement identique.

```ocara
scoped parts:string[] = String::explode("x|y|z", "|")
// → ["x", "y", "z"]
```

---

### `String::between(s, start, end)` → `string`

Extrait la sous-chaîne de `s` comprise **entre** la première occurrence de `start`
et la première occurrence de `end` qui la suit.  
Retourne une chaîne vide si `start` ou `end` est introuvable.

| Paramètre | Type     | Description          |
|-----------|----------|----------------------|
| `s`       | `string` | Chaîne source        |
| `start`   | `string` | Délimiteur d'ouverture |
| `end`     | `string` | Délimiteur de fermeture |

```ocara
String::between("version:[1.0]fin", "[", "]")   // → "1.0"
String::between("<b>texte</b>", "<b>", "</b>")   // → "texte"
```

---

### `String::empty(s)` → `bool`

Retourne `true` si `s` est vide ou ne contient que des espaces.  
Équivalent à `String::len(String::trim(s)) == 0`.

```ocara
String::empty("")        // → true
String::empty("   ")     // → true
String::empty("a")       // → false
String::empty(" a ")     // → false
```

---

## Combinaisons courantes

```ocara
import ocara.String

function main(): int {

    // Normaliser une saisie utilisateur
    var saisie:string = "  Alice  "
    scoped nom:string = String::capitalize(String::trim(saisie))
    write(`Bonjour ${nom} !`)     // Bonjour Alice !

    // Vérifier avant d'utiliser
    var entree:string = ""
    if String::empty(entree) {
        entree = "valeur par défaut"
    }

    // Découper et afficher
    scoped tags:string[] = String::split("rust,ocara,cranelift", ",")
    write(`Premier tag : ${String::upper(tags[0])}`)   // RUST

    // Extraire une donnée balisée
    scoped version:string = String::between("build:[2.0.1]:stable", "[", "]")
    write(`Version = ${version}`)   // 2.0.1

    return 0
}
```

---

## Gestion d'erreurs

**Toutes les méthodes String sont "safe"** et ne lèvent jamais d'exception.

Les cas limites retournent des valeurs sensées :
- `String::len()` retourne toujours un nombre (0 pour chaîne vide)
- `String::between()` retourne une chaîne vide si les délimiteurs ne sont pas trouvés
- `String::split()` retourne toujours un tableau (vide si chaîne vide)
- `String::empty()` retourne toujours `true` ou `false`

Aucune StringException n'est actuellement définie ou levée par les méthodes de la classe String.

**Remarque :** Le typage statique d'Ocara garantit que vous ne passerez jamais un type incorrect à une méthode String (erreur de compilation).

---

## Conventions runtime

Les méthodes sont implémentées côté runtime C sous le préfixe `String_` :

| Méthode Ocara            | Symbole runtime C        |
|--------------------------|--------------------------|
| `String::len`            | `String_len`             |
| `String::upper`          | `String_upper`           |
| `String::lower`          | `String_lower`           |
| `String::capitalize`     | `String_capitalize`      |
| `String::trim`           | `String_trim`            |
| `String::replace`        | `String_replace`         |
| `String::split`          | `String_split`           |
| `String::explode`        | `String_explode`         |
| `String::between`        | `String_between`         |
| `String::empty`          | `String_empty`           |

---

## Voir aussi

- [examples/builtins/string.oc](../../examples/builtins/string.oc) — exemple complet exécutable
- [docs/EBNF.md](../EBNF.md) — grammaire formelle (section `ImportDecl`)
