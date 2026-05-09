# ocara.MySQL / ocara.MariaDB

Classe builtin pour interagir avec des bases de données MySQL et MariaDB.

> **Note** : `MariaDB` est un alias complet de `MySQL`. Toutes les fonctionnalités sont identiques. Vous pouvez utiliser `import ocara.MySQL` ou `import ocara.MariaDB` de manière interchangeable.

## Import

```ocara
import ocara.MySQL
// ou
import ocara.MariaDB
```

## Connexion à une base de données

### `MySQL::connect(host: string, user: string, password: string, database: string) → MySQL`

Établit une connexion à un serveur MySQL.

```ocara
const db:MySQL = MySQL::connect("localhost", "root", "password", "mydb")
```

**Paramètres** :
- `host` : adresse du serveur MySQL (ex: `"localhost"`, `"127.0.0.1"`)
- `user` : nom d'utilisateur
- `password` : mot de passe
- `database` : nom de la base de données

**Erreur** : `MySQLException` (code 101) si impossible de se connecter.

## Exécution de requêtes

### `db.execute(query: string) → int`

Exécute une requête SQL qui ne retourne pas de résultats (INSERT, UPDATE, DELETE, CREATE, etc.).  
Retourne le nombre de lignes affectées.

```ocara
db.execute("CREATE TABLE users (id INT PRIMARY KEY AUTO_INCREMENT, name VARCHAR(100), age INT)")
db.execute("INSERT INTO users (name, age) VALUES ('Alice', 30)")
const affected:int = db.execute("UPDATE users SET age = 31 WHERE name = 'Alice'")
IO::writeln(`${affected} rows updated`)
```

**Erreur** : `MySQLException` (code 102) si la requête échoue.

### `db.query(query: string) → array<map<string, mixed>>`

Exécute une requête SELECT et retourne un array de maps (une map par ligne).

```ocara
const rows:array<map<string, mixed>> = db.query("SELECT * FROM users")

for row in rows {
    const name:string = row["name"]
    const age:int = row["age"]
    IO::writeln(`${name} has ${age} years`)
}
```

**Types de colonnes** :
- `INT`, `BIGINT` → `int` (i64)
- `FLOAT`, `DOUBLE` → `float` (f64)
- `VARCHAR`, `TEXT` → `string`
- `NULL` → `null` (0)

**Erreur** : `MySQLException` (code 103) si la requête échoue.

### `db.queryOne(query: string) → map<string, mixed>|null`

Exécute une requête SELECT et retourne la première ligne, ou `null` si aucun résultat.

```ocara
const user:map<string, mixed> = db.queryOne("SELECT * FROM users WHERE id = 1")

if user != null {
    IO::writeln(`User found: ${user["name"]}`)
} else {
    IO::writeln("User not found")
}
```

**Erreur** : `MySQLException` (code 103) si la requête échoue.

## Informations sur les opérations

### `db.lastInsertId() → int`

Retourne l'ID de la dernière insertion (AUTO_INCREMENT).

```ocara
db.execute("INSERT INTO users (name, age) VALUES ('Bob', 25)")
const id:int = db.lastInsertId()
IO::writeln(`New user ID: ${id}`)
```

### `db.affectedRows() → int`

Retourne le nombre de lignes affectées par la dernière requête `execute()`.

```ocara
const affected:int = db.execute("UPDATE users SET age = 32 WHERE name = 'Alice'")
IO::writeln(`${affected} rows updated`)
```

## Fermeture

### `db.close() → void`

Ferme la connexion à la base de données.

```ocara
db.close()
```

## Exemple complet

```ocara
import ocara.MySQL
import ocara.IO

const db:MySQL = MySQL::connect("localhost", "root", "password", "testdb")

try {
    // Création de table
    db.execute("CREATE TABLE IF NOT EXISTS users (
        id INT PRIMARY KEY AUTO_INCREMENT,
        name VARCHAR(100) NOT NULL,
        email VARCHAR(100),
        age INT
    )")
    
    // Insertion
    db.execute("INSERT INTO users (name, email, age) VALUES ('Alice', 'alice@example.com', 30)")
    db.execute("INSERT INTO users (name, email, age) VALUES ('Bob', 'bob@example.com', 25)")
    
    const lastId:int = db.lastInsertId()
    IO::writeln(`Last inserted ID: ${lastId}`)
    
    // Requête SELECT
    const users:array<map<string, mixed>> = db.query("SELECT * FROM users WHERE age > 20")
    
    IO::writeln(`Found ${Array::length(users)} users:`)
    for user in users {
        IO::writeln(`- ${user["name"]} (${user["age"]} years) - ${user["email"]}`)
    }
    
    // Mise à jour
    const affected:int = db.execute("UPDATE users SET age = 31 WHERE name = 'Alice'")
    IO::writeln(`${affected} rows updated`)
    
    // Recherche d'un utilisateur
    const alice:map<string, mixed> = db.queryOne("SELECT * FROM users WHERE name = 'Alice'")
    if alice != null {
        IO::writeln(`Alice's new age: ${alice["age"]}`)
    }
    
    db.close()
    
} fail (e:MySQLException) {
    IO::writeln(`MySQL error: ${e.message}`)
}

// Fonctionne aussi avec MariaDB:
import ocara.MariaDB
const db2:MariaDB = MariaDB::connect("localhost", "root", "password", "testdb")
// ... même API ...
```

## Notes

- La connexion utilise un pool de connexions en interne pour de meilleures performances
- Les types MySQL sont convertis automatiquement en types Ocara
- `NULL` en MySQL devient `null` (0) en Ocara
- Pour les chaînes avec caractères spéciaux, utilisez des paramètres préparés ou échappez les valeurs
- La connexion doit être fermée avec `close()` pour libérer les ressources
