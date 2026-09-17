# ocara.SQLite

Classe builtin pour interagir avec des bases de données SQLite.

## Import

```ocara
import ocara.SQLite
```

## Ouverture d'une base de données

### `SQLite::open(path: string) → SQLite`

Ouvre ou crée une base de données SQLite.

```ocara
const db:SQLite = SQLite::open("data.db")
```

**Erreur** : `SQLiteException` (code 101) si impossible d'ouvrir la base.

### `SQLite::withOpen(path: string, f: Function<void(SQLite)>) → void`

Ouvre la base, exécute `f(db)` avec la connexion fraîchement ouverte, puis **ferme systématiquement** la connexion — y compris si `f()` lève une exception. Avec `open()`/`close()` manuels, un `raise` entre les deux appels saute `close()` (le mécanisme d'exceptions d'Ocara est `setjmp`/`longjmp`, sans unwinding) et fuit la connexion pour toujours ; c'est exactement le risque que le diagnostic [W04](../diagnostics.md) signale statiquement pour une ressource `scoped`/`consumed`, et que `withOpen` corrige à la racine pour qui l'utilise.

```ocara
SQLite::withOpen("data.db", nameless(db:SQLite): void {
    db.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
    db.execute("INSERT INTO users (name) VALUES ('Alice')")
    // db est fermée automatiquement ici, même si une exception est levée
    // au-dessus de cette ligne.
})
```

**Erreur** : `SQLiteException` (code 101) si impossible d'ouvrir la base ; toute exception levée par `f()` continue de se propager normalement à l'appelant, après la fermeture de la connexion.

## Exécution de requêtes

### `db.execute(query: string, placeholder: map<string, mixed>|null = null, close: bool = false) → void`

Exécute une requête SQL qui ne retourne pas de résultats (INSERT, UPDATE, DELETE, CREATE, etc.).

```ocara
db.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)")
db.execute("INSERT INTO users (name, age) VALUES ('Alice', 30)")
db.execute("UPDATE users SET age = 31 WHERE name = 'Alice'")
db.execute("DELETE FROM users WHERE id = 1")
```

`placeholder` bind des valeurs nominatives `:nom` (voir [Requêtes paramétrées](#requêtes-paramétrées-placeholders-nominatifs) ci-dessous) — pas de requête préparée nécessaire pour un simple binding one-shot :

```ocara
db.execute("INSERT INTO users (name, age) VALUES (:name, :age)", {"name": "Alice", "age": 30})
```

`close:true` ferme la connexion juste après (pratique pour un appel unique en fin de fonction) :

```ocara
db.execute("VACUUM", null, true)
```

> ⚠️ L'analyse statique de fuite de ressource (diagnostic E28, voir [diagnostics.md](../diagnostics.md)) ne reconnaît que l'appel explicite `.close()` — passer `close:true` ferme réellement la connexion à l'exécution, mais **ne satisfait pas** ce diagnostic. Préférer `.close()` explicite si vous voulez que le compilateur valide qu'aucune connexion ne fuit.

**Erreur** : `SQLiteException` (code 102) si la requête échoue.

### `db.query(query: string, placeholder: map<string, mixed>|null = null) → array<map<string, mixed>>`

Exécute une requête SELECT et retourne un array de maps (une map par ligne).

```ocara
const rows:array<map<string, mixed>> = db.query("SELECT * FROM users")
const adults:array<map<string, mixed>> = db.query("SELECT * FROM users WHERE age >= :min", {"min": 18})

for row in rows {
    const name:string = row["name"]
    const age:int = row["age"]
    IO::writeln(`${name} has ${age} years`)
}
```

**Erreur** : `SQLiteException` (code 103) si la requête échoue.

### `db.queryOne(query: string, placeholder: map<string, mixed>|null = null) → map<string, mixed>`

Exécute une requête SELECT et retourne une seule ligne (ou une map vide si aucun résultat).

> ⚠️ **Piège** : `MySQL::queryOne`/`MariaDB::queryOne` (voir [MySQL](MySQL.md)) portent le même nom de méthode mais une sémantique **différente** pour représenter "aucun résultat" — ils retournent `null` (type `map<string, mixed>|null`), pas une map vide. Ne pas écrire de code générique sur les deux sans tenir compte de cette différence.

```ocara
const user:map<string, mixed> = db.queryOne("SELECT * FROM users WHERE id = :id", {"id": 1})

if Map::size(user) > 0 {
    IO::writeln(`User found: ${user["name"]}`)
} else {
    IO::writeln("User not found")
}
```

**Erreur** : `SQLiteException` (code 103) si la requête échoue.

## Requêtes paramétrées (placeholders nominatifs)

Toute valeur variable dans une requête SQL doit être **bindée**, jamais concaténée dans la chaîne — la concaténation directe est une injection SQL potentielle. `SQLite` n'accepte que des placeholders **nominatifs**, écrits `:nom` dans le SQL, liés depuis une `map<string, mixed>` dont les clés sont les mêmes noms **sans** le `:` :

```ocara
db.execute("INSERT INTO users (name, age) VALUES (:name, :age)", {"name": "Alice", "age": 30})
```

Il n'y a pas de placeholders positionnels (`?`) — choix délibéré, cohérent avec le reste du langage (comparaisons en toutes lettres plutôt qu'en symboles, voir `examples/32_strict_operators.oc`) : le nom du placeholder porte l'intention au point d'appel, un `?` ne le fait pas.

Types de valeurs bindables : `int`, `float`, `bool`, `string`, `null`. Une valeur `array`/`map`/objet/fonction dans la map de placeholders lève `SQLiteException` (code 106) — ce ne sont pas des valeurs SQL scalaires.

## Transactions — `prepare()` / `bind()` / `commit()` / `rollback()`

Pour une opération qui doit réussir ou échouer **en bloc** (donc annulable via `rollback()`), le flux "stepped" remplace `execute`/`query`/`queryOne` one-shot par quatre étapes explicites sur la même connexion :

```ocara
const db:SQLite = SQLite::open("data.db")
try {
    db.prepare("UPDATE accounts SET balance = balance - :amount WHERE id = :from")
    db.bind({"amount": 100, "from": 1})
    db.commit()

    db.prepare("UPDATE accounts SET balance = balance + :amount WHERE id = :to")
    db.bind({"amount": 100, "to": 2})
    var affected:mixed = db.commit()
    IO::writeln(`transfert effectué, ${affected} ligne(s) affectée(s)`)
} on e is SQLiteException {
    IO::writeln(`Erreur : ${e.message}`)
    db.rollback()
}
```

### `db.prepare(query: string) → void`

Valide la requête **immédiatement** (erreur de syntaxe remontée à cet appel, pas plus tard) et la mémorise pour `bind()`/`commit()`. N'exécute rien.

**Erreur** : `SQLiteException` (code 105) si la requête est syntaxiquement invalide.

### `db.bind(placeholders: map<string, mixed>) → void`

Bind les placeholders nominatifs `:nom` de la requête posée par le dernier `prepare()`. Doit suivre un `prepare()` sans `commit()`/`rollback()` entre les deux — appeler `bind()` sans `prepare()` préalable lève une exception plutôt que d'échouer silencieusement.

**Erreur** : `SQLiteException` (code 106) si aucun `prepare()` n'est en attente, ou si une valeur de placeholder n'est pas bindable.

### `db.commit(close: bool = false) → mixed`

Exécute la requête posée par `prepare()`/`bind()` à l'intérieur d'une transaction SQL (`BEGIN` ... `COMMIT`). Le type de retour dépend de la requête, **auto-détecté** :
- `array<map<string, mixed>>` si la requête produit des colonnes (SELECT) ;
- `int` (nombre de lignes affectées) sinon (INSERT/UPDATE/DELETE/CREATE...).

```ocara
db.prepare("SELECT * FROM users WHERE age > :min")
db.bind({"min": 18})
var result:mixed = db.commit()
var rows:array<map<string, mixed>> = result   // narrower explicite, mixed désactive le typage statique
```

En cas d'échec à n'importe quelle étape (BEGIN, exécution, COMMIT), lève `SQLiteException` **sans rollback automatique** : la transaction reste ouverte, `rollback()` doit être appelé explicitement pour l'annuler (voir l'exemple de flux ci-dessus). Appeler `commit()` sans `prepare()` préalable lève aussi une exception.

**Erreur** : `SQLiteException` (code 107).

### `db.rollback(close: bool = false) → void`

Annule la transaction laissée ouverte par un `commit()` échoué, et efface l'état `prepare()`/`bind()` en attente. **No-op silencieux** s'il n'y a rien à annuler — peut être appelé sans risque depuis un `on e is SQLiteException` sans savoir précisément à quelle étape l'échec a eu lieu. Un échec du `ROLLBACK` lui-même (rare) est absorbé silencieusement plutôt que de lever une nouvelle exception qui masquerait celle d'origine.

Comme `commit()`, `close:bool = false` — fermeture de la connexion optionnelle, jamais implicite (voir l'avertissement sur E28 dans la section `execute()` ci-dessus : ceci vaut aussi pour `commit`/`rollback`).

## Informations sur les opérations

### `db.lastInsertId() → int`

Retourne l'ID de la dernière insertion (rowid).

```ocara
db.execute("INSERT INTO users (name, age) VALUES ('Bob', 25)")
const id:int = db.lastInsertId()
IO::writeln(`New user ID: ${id}`)
```

### `db.affectedRows() → int`

Retourne le nombre de lignes affectées par la dernière requête.

```ocara
db.execute("UPDATE users SET age = 32 WHERE name = 'Alice'")
const affected:int = db.affectedRows()
IO::writeln(`${affected} rows updated`)

const rows:array<map<string, mixed>> = db.query("SELECT * FROM users")
const count:int = db.affectedRows()
IO::writeln(`${count} rows returned`)
```

## Fermeture

### `db.close() → void`

Ferme la connexion à la base de données.

```ocara
db.close()
```

**Note** : La connexion est automatiquement fermée quand l'objet est détruit, mais il est recommandé d'appeler `close()` explicitement.

## Gestion d'erreurs

Toutes les erreurs SQLite lèvent une `SQLiteException` avec :
- `message` : Description de l'erreur
- `code` : Code d'erreur (101-107, voir tableau ci-dessous)
- `source` : "SQLite"

```ocara
try {
    const db:SQLite = SQLite::open("data.db")
    db.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
    db.execute("INSERT INTO users (name) VALUES ('Alice')")
    db.close()
} on e is SQLiteException {
    IO::writeln(`SQLite error (${e.code}): ${e.message}`)
}
```

## Codes d'erreur

| Code | Nom | Description |
|------|-----|-------------|
| 101  | OPEN | Erreur d'ouverture de la base de données |
| 102  | EXECUTE | Erreur d'exécution d'une requête |
| 103  | QUERY | Erreur d'exécution d'un SELECT |
| 104  | CLOSE | Erreur de fermeture de la connexion (réservé, non utilisé actuellement) |
| 105  | PREPARE | Erreur de syntaxe/préparation d'une requête (`prepare()`) |
| 106  | BIND | Placeholder inconnu, valeur non bindable, ou `bind()` sans `prepare()` |
| 107  | COMMIT | Erreur pendant BEGIN/exécution/COMMIT, ou `commit()` sans `prepare()` |

`rollback()` n'a volontairement pas de code dédié : un échec du `ROLLBACK` lui-même est absorbé silencieusement (voir plus haut) pour ne jamais masquer l'exception d'origine.

## Exemple complet

```ocara
import ocara.SQLite
import ocara.IO

init {
    try {
        // Ouvrir la base
        const db:SQLite = SQLite::open("test.db")
        
        // Créer une table
        db.execute("CREATE TABLE IF NOT EXISTS products (id INTEGER PRIMARY KEY, name TEXT, price REAL)")
        
        // Insérer des données
        db.execute("INSERT INTO products (name, price) VALUES ('Laptop', 999.99)")
        db.execute("INSERT INTO products (name, price) VALUES ('Mouse', 29.99)")
        db.execute("INSERT INTO products (name, price) VALUES ('Keyboard', 79.99)")
        
        IO::writeln(`Last insert ID: ${db.lastInsertId()}`)
        
        // Requête
        const products:array<map<string, mixed>> = db.query("SELECT * FROM products WHERE price > 50")
        
        IO::writeln(`Found ${db.affectedRows()} products:`)
        for product in products {
            IO::writeln(`  - ${product["name"]}: $${product["price"]}`)
        }
        
        // Mettre à jour
        db.execute("UPDATE products SET price = 899.99 WHERE name = 'Laptop'")
        IO::writeln(`Updated ${db.affectedRows()} rows`)
        
        // Fermer
        db.close()
        
    } on e is SQLiteException {
        IO::writeln(`Database error: ${e.message}`)
        return e.code
    }
}
```

## Exemple — requêtes paramétrées et transaction

```ocara
import ocara.SQLite
import ocara.IO

function main(): int {
    const db:SQLite = SQLite::open("bank.db")

    db.execute("CREATE TABLE IF NOT EXISTS accounts (id INTEGER PRIMARY KEY, balance INTEGER)")

    // One-shot avec binding nominatif — jamais de concaténation de chaîne
    db.execute("INSERT OR IGNORE INTO accounts (id, balance) VALUES (:id, :balance)", {"id": 1, "balance": 500})
    db.execute("INSERT OR IGNORE INTO accounts (id, balance) VALUES (:id, :balance)", {"id": 2, "balance": 100})

    // Transaction : virement qui doit réussir ou échouer en bloc
    try {
        db.prepare("UPDATE accounts SET balance = balance - :amount WHERE id = :from")
        db.bind({"amount": 50, "from": 1})
        db.commit()

        db.prepare("UPDATE accounts SET balance = balance + :amount WHERE id = :to")
        db.bind({"amount": 50, "to": 2})
        db.commit()

        IO::writeln("Virement effectué")
    } on e is SQLiteException {
        IO::writeln(`Virement annulé : ${e.message}`)
        db.rollback()
    }

    const accounts:array<map<string, mixed>> = db.query("SELECT * FROM accounts")
    for a in accounts {
        IO::writeln(`compte ${a["id"]} : ${a["balance"]}`)
    }

    db.close()
    return 0
}
```
