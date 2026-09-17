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

### `MySQL::withConnect(host: string, user: string, password: string, database: string, f: Function<void(MySQL)>) → void`

Connecte, exécute `f(db)` avec la connexion fraîchement établie, puis **ferme systématiquement** la connexion — y compris si `f()` lève une exception. Avec `connect()`/`close()` manuels, un `raise` entre les deux appels saute `close()` (le mécanisme d'exceptions d'Ocara est `setjmp`/`longjmp`, sans unwinding) et fuit la connexion pour toujours ; c'est exactement le risque que le diagnostic [W04](../diagnostics.md) signale statiquement pour une ressource `scoped`/`consumed`, et que `withConnect` corrige à la racine pour qui l'utilise. Disponible également sous `MariaDB::withConnect`.

```ocara
MySQL::withConnect("localhost", "root", "password", "mydb", nameless(db:MySQL): void {
    db.execute("CREATE TABLE IF NOT EXISTS users (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255))")
    db.execute("INSERT INTO users (name) VALUES ('Alice')")
    // db est fermée automatiquement ici, même si une exception est levée
    // au-dessus de cette ligne.
})
```

**Erreur** : `MySQLException` (code 101) si impossible de se connecter ; toute exception levée par `f()` continue de se propager normalement à l'appelant, après la fermeture de la connexion.

## Exécution de requêtes

### `db.execute(query: string, placeholder: map<string, mixed>|null = null, close: bool = false) → int`

Exécute une requête SQL qui ne retourne pas de résultats (INSERT, UPDATE, DELETE, CREATE, etc.).  
Retourne le nombre de lignes affectées.

```ocara
db.execute("CREATE TABLE users (id INT PRIMARY KEY AUTO_INCREMENT, name VARCHAR(100), age INT)")
db.execute("INSERT INTO users (name, age) VALUES ('Alice', 30)")
const affected:int = db.execute("UPDATE users SET age = 31 WHERE name = 'Alice'")
IO::writeln(`${affected} rows updated`)
```

`placeholder` bind des valeurs nominatives `:nom` (voir [Requêtes paramétrées](#requêtes-paramétrées-placeholders-nominatifs) ci-dessous) :

```ocara
db.execute("INSERT INTO users (name, age) VALUES (:name, :age)", {"name": "Alice", "age": 30})
```

`close:true` ferme la connexion juste après. Comme côté [SQLite](SQLite.md), ceci ferme réellement la connexion à l'exécution mais ne satisfait **pas** l'analyse statique de fuite de ressource (E28, voir [diagnostics.md](../diagnostics.md)) — préférer `.close()` explicite si vous voulez que le compilateur valide qu'aucune connexion ne fuit.

**Erreur** : `MySQLException` (code 102) si la requête échoue.

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

**Types de colonnes** :
- `INT`, `BIGINT` → `int` (i64)
- `FLOAT`, `DOUBLE` → `float` (f64)
- `VARCHAR`, `TEXT` → `string`
- `NULL` → `null` (0)

**Erreur** : `MySQLException` (code 103) si la requête échoue.

### `db.queryOne(query: string, placeholder: map<string, mixed>|null = null) → map<string, mixed>|null`

Exécute une requête SELECT et retourne la première ligne, ou `null` si aucun résultat.

> ⚠️ **Piège** : `SQLite::queryOne` (voir [SQLite](SQLite.md)) porte le même nom de méthode mais une sémantique **différente** pour représenter "aucun résultat" — il retourne une **map vide** (vérifier avec `Map::size(...) > 0`), pas `null`. Ne pas écrire de code générique sur les deux sans tenir compte de cette différence.

```ocara
const user:map<string, mixed>|null = db.queryOne("SELECT * FROM users WHERE id = :id", {"id": 1})

if user not equal null {
    IO::writeln(`User found: ${user["name"]}`)
} else {
    IO::writeln("User not found")
}
```

**Erreur** : `MySQLException` (code 103) si la requête échoue.

## Requêtes paramétrées (placeholders nominatifs)

Toute valeur variable dans une requête SQL doit être **bindée**, jamais concaténée dans la chaîne — la concaténation directe est une injection SQL potentielle. Comme [SQLite](SQLite.md), `MySQL`/`MariaDB` n'acceptent que des placeholders **nominatifs**, écrits `:nom` dans le SQL, liés depuis une `map<string, mixed>` dont les clés sont les mêmes noms **sans** le `:` :

```ocara
db.execute("INSERT INTO users (name, age) VALUES (:name, :age)", {"name": "Alice", "age": 30})
```

Pas de placeholders positionnels (`?`) — même choix que SQLite, voir `examples/32_strict_operators.oc` pour la même philosophie appliquée aux opérateurs de comparaison.

Types de valeurs bindables : `int`, `float`, `bool` (converti en `0`/`1`, MySQL n'a pas de type bool natif), `string`, `null`. Une valeur `array`/`map`/objet/fonction dans la map de placeholders lève `MySQLException` (code 106).

## Transactions — `prepare()` / `bind()` / `commit()` / `rollback()`

Pour une opération qui doit réussir ou échouer **en bloc** (donc annulable via `rollback()`), le flux "stepped" remplace `execute`/`query`/`queryOne` one-shot par quatre étapes explicites — même design que [SQLite](SQLite.md#transactions--prepare--bind--commit--rollback), sur la même connexion à chaque étape :

```ocara
const db:MySQL = MySQL::connect("localhost", "root", "password", "bank")
try {
    db.prepare("UPDATE accounts SET balance = balance - :amount WHERE id = :from")
    db.bind({"amount": 100, "from": 1})
    db.commit()

    db.prepare("UPDATE accounts SET balance = balance + :amount WHERE id = :to")
    db.bind({"amount": 100, "to": 2})
    var affected:mixed = db.commit()
    IO::writeln(`transfert effectué, ${affected} ligne(s) affectée(s)`)
} on e is MySQLException {
    IO::writeln(`Erreur : ${e.message}`)
    db.rollback()
}
```

### `db.prepare(query: string) → void`

Épingle une connexion dédiée au cycle et valide la requête **immédiatement** côté serveur (`COM_STMT_PREPARE` réel — erreur de syntaxe remontée à cet appel, pas plus tard). Un cycle précédent resté ouvert (`commit()` échoué, `rollback()` jamais appelé) est annulé automatiquement avant de continuer.

**Erreur** : `MySQLException` (code 105) si la requête est syntaxiquement invalide.

### `db.bind(placeholders: map<string, mixed>) → void`

Bind les placeholders nominatifs `:nom` de la requête posée par le dernier `prepare()`. Appeler `bind()` sans `prepare()` préalable lève une exception plutôt que d'échouer silencieusement.

**Erreur** : `MySQLException` (code 106) si aucun `prepare()` n'est en attente, ou si une valeur n'est pas bindable.

### `db.commit(close: bool = false) → mixed`

Exécute la requête posée par `prepare()`/`bind()` à l'intérieur d'une transaction SQL (`START TRANSACTION` ... `COMMIT`), sur la connexion épinglée par `prepare()`. Le type de retour dépend de la requête, **auto-détecté** :
- `array<map<string, mixed>>` si la requête produit des colonnes (SELECT) ;
- `int` (nombre de lignes affectées) sinon.

En cas d'échec à n'importe quelle étape, lève `MySQLException` **sans rollback automatique** : `rollback()` doit être appelé explicitement. Appeler `commit()` sans `prepare()` préalable lève aussi une exception.

**Erreur** : `MySQLException` (code 107).

### `db.rollback(close: bool = false) → void`

Annule la transaction laissée ouverte par un `commit()` échoué, et efface l'état `prepare()`/`bind()` en attente. **No-op silencieux** s'il n'y a rien à annuler — peut être appelé sans risque depuis un `on e is MySQLException` sans savoir précisément à quelle étape l'échec a eu lieu.

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
    
    IO::writeln(`Found ${Array::len(users)} users:`)
    for user in users {
        IO::writeln(`- ${user["name"]} (${user["age"]} years) - ${user["email"]}`)
    }
    
    // Mise à jour
    const affected:int = db.execute("UPDATE users SET age = 31 WHERE name = 'Alice'")
    IO::writeln(`${affected} rows updated`)
    
    // Recherche d'un utilisateur
    const alice:map<string, mixed>|null = db.queryOne("SELECT * FROM users WHERE name = 'Alice'")
    if alice not equal null {
        IO::writeln(`Alice's new age: ${alice["age"]}`)
    }
    
    db.close()
    
} on e is MySQLException {
    IO::writeln(`MySQL error: ${e.message}`)
}

// Fonctionne aussi avec MariaDB:
import ocara.MariaDB
const db2:MariaDB = MariaDB::connect("localhost", "root", "password", "testdb")
// ... même API ...
```

## Exemple — requêtes paramétrées et transaction

```ocara
import ocara.MySQL
import ocara.IO

function main(): int {
    const db:MySQL = MySQL::connect("localhost", "root", "password", "bank")

    db.execute("CREATE TABLE IF NOT EXISTS accounts (id INT AUTO_INCREMENT PRIMARY KEY, balance INT)")

    // One-shot avec binding nominatif — jamais de concaténation de chaîne
    db.execute("INSERT IGNORE INTO accounts (id, balance) VALUES (:id, :balance)", {"id": 1, "balance": 500})
    db.execute("INSERT IGNORE INTO accounts (id, balance) VALUES (:id, :balance)", {"id": 2, "balance": 100})

    // Transaction : virement qui doit réussir ou échouer en bloc
    try {
        db.prepare("UPDATE accounts SET balance = balance - :amount WHERE id = :from")
        db.bind({"amount": 50, "from": 1})
        db.commit()

        db.prepare("UPDATE accounts SET balance = balance + :amount WHERE id = :to")
        db.bind({"amount": 50, "to": 2})
        db.commit()

        IO::writeln("Virement effectué")
    } on e is MySQLException {
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

## Notes

- La connexion utilise un pool de connexions en interne pour de meilleures performances (sauf pendant un cycle `prepare()`/`bind()`/`commit()`/`rollback()`, qui épingle une connexion dédiée le temps de la transaction — voir la section Transactions ci-dessus)
- Les types MySQL sont convertis automatiquement en types Ocara
- `NULL` en MySQL devient `null` (0) en Ocara
- Toute valeur variable doit être bindée via le paramètre `placeholder`/`bind()` (voir [Requêtes paramétrées](#requêtes-paramétrées-placeholders-nominatifs)) — jamais concaténée directement dans la chaîne SQL
- La connexion doit être fermée avec `close()` pour libérer les ressources
