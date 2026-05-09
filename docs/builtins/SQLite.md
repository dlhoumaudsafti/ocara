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

## Exécution de requêtes

### `db.execute(query: string) → void`

Exécute une requête SQL qui ne retourne pas de résultats (INSERT, UPDATE, DELETE, CREATE, etc.).

```ocara
db.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)")
db.execute("INSERT INTO users (name, age) VALUES ('Alice', 30)")
db.execute("UPDATE users SET age = 31 WHERE name = 'Alice'")
db.execute("DELETE FROM users WHERE id = 1")
```

**Erreur** : `SQLiteException` (code 102) si la requête échoue.

### `db.query(query: string) → map<string, mixed>[]`

Exécute une requête SELECT et retourne un array de maps (une map par ligne).

```ocara
const rows:map<string, mixed>[] = db.query("SELECT * FROM users")

for row in rows {
    const name:string = row["name"]
    const age:int = row["age"]
    IO::writeln(`${name} has ${age} years`)
}
```

**Erreur** : `SQLiteException` (code 103) si la requête échoue.

### `db.queryOne(query: string) → map<string, mixed>`

Exécute une requête SELECT et retourne une seule ligne (ou une map vide si aucun résultat).

```ocara
const user:map<string, mixed> = db.queryOne("SELECT * FROM users WHERE id = 1")

if Map::size(user) > 0 {
    IO::writeln(`User found: ${user["name"]}`)
} else {
    IO::writeln("User not found")
}
```

**Erreur** : `SQLiteException` (code 103) si la requête échoue.

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

const rows:map<string, mixed>[] = db.query("SELECT * FROM users")
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
- `code` : Code d'erreur (101-104)
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
| 104  | CLOSE | Erreur de fermeture de la connexion |

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
        const products:map<string, mixed>[] = db.query("SELECT * FROM products WHERE price > 50")
        
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
