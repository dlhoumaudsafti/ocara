# `ocara.Exception` — Classes d'exception builtin

> Classe de base `Exception` et ses sous-classes builtin (`FileException`, `MapException`…), levées par le runtime ou par le programme (`raise`).  
> Toutes partagent les mêmes champs et le même constructeur.

---

## Import

```ocara
import ocara.Exception      // facultatif
import ocara.*              // importe toutes les classes builtins
```

Les classes d'exception builtin sont connues du compilateur sans import : `on e is FileException`, `raise use Exception("…")` ou `class MyErr extends Exception` compilent tels quels.

---

## Champs

| Champ     | Type     | Description |
|-----------|----------|-------------|
| `message` | `string` | Description de l'erreur |
| `code`    | `int`    | Code d'erreur (`0` = pas de code) |
| `source`  | `string` | Module d'origine (`"File"`, `"Map"`, `"HTTPServer"`…) pour une exception levée par le runtime ; `""` pour une exception construite par le programme |

Les accès aux champs sont vérifiés à la compilation : `e.message()` (champ appelé comme une méthode) est rejeté.

---

## Constructeur

### `use Exception(message:string, code:int = 0)` → `Exception`

Même constructeur pour toutes les classes builtin de la liste ci-dessous.

```ocara
raise use Exception("échec générique")          // e.code = 0, e.source = ""
raise use FileException("disque plein", 28)     // e.message = "disque plein", e.code = 28
```

---

## Classes builtin

Chaque classe ci-dessous est un **sous-type de `Exception`** :

| Classe | Levée par |
|--------|-----------|
| `FileException` | [File](File.md) |
| `DirectoryException` | [Directory](Directory.md) |
| `IOException` | [IO](IO.md) |
| `SystemException` | [System](System.md) |
| `ArrayException` | [Array](Array.md) |
| `MapException` | [Map](Map.md) |
| `MathException` | [Math](Math.md) |
| `ConvertException` | [Convert](Convert.md) |
| `RegexException` | [Regex](Regex.md) |
| `DateTimeException` | [DateTime](DateTime.md) |
| `DateException` | [Date](Date.md) |
| `TimeException` | [Time](Time.md) |
| `ThreadException` | [Thread](Thread.md) |
| `MutexException` | [Mutex](Mutex.md) |
| `UnitTestException` | [UnitTest](UnitTest.md) |
| `HTTPServerException` | [HTTPServer](HTTPServer.md) |
| `SQLiteException` | [SQLite](SQLite.md) |
| `MySQLException` | [MySQL](MySQL.md) |
| `MariaDBException` | [MySQL](MySQL.md) |
| `DotEnvException` | [DotEnv](DotEnv.md) |
| `YAMLException` | [YAML](YAML.md) |
| `SDLException` | [SDL](SDL.md) |
| `ImageException` | [Image](Image.md) |
| `AndroidException` | [AndroidPermissions](AndroidPermissions.md), [AndroidNotifications](AndroidNotifications.md), [AndroidSensors](AndroidSensors.md), [AndroidFiles](AndroidFiles.md), [AndroidCamera](AndroidCamera.md), [AndroidMicrophone](AndroidMicrophone.md), [AndroidAudio](AndroidAudio.md) |
| `TauriException` | [Tauri](Tauri.md) |

Conséquences du sous-typage :

- `on e is Exception` attrape **n'importe laquelle** de ces exceptions, qu'elle soit levée par le runtime (ex. `File::read` sur un fichier inexistant) ou par `raise`.
- Une `FileException` (ou toute autre) peut être passée à un paramètre de type `Exception`.

```ocara
function describe(e:Exception): string {
    return `[${e.code}] ${e.message} (source: "${e.source}")`
}

try {
    File::read("/inexistant/fichier.txt")
} on e is FileException {
    IO::writeln(describe(e))
    // [101] Failed to read file '/inexistant/fichier.txt': No such file or directory (os error 2) (source: "File")
}
```

---

## Exception personnalisée

Une classe qui étend `Exception` déclare un `init` qui transmet ses arguments avec `parent::init(...)`. Sans `init`, `use MyErr("…", 1)` est refusé (E60 : la classe n'a pas de constructeur acceptant des arguments).

```ocara
class AppError extends Exception {
    init(message:string, code:int) {
        parent::init(message, code)
    }
}

class HttpError extends Exception {
    public property status:int

    init(message:string, status:int) {
        parent::init(message, 500)
        self.status = status
    }
}
```

Une sous-classe utilisateur hérite elle aussi de `Exception` : `on e is Exception` l'attrape.

---

## `try` / `on`

| Forme | Attrape | Type de `e` |
|-------|---------|-------------|
| `on e is X` | Une instance de `X` ou d'une sous-classe de `X` | `X` (champs vérifiés à la compilation) |
| `on e` | Toute erreur (doit être placé en dernier) | `mixed` |

```ocara
try {
    raise use HttpError("introuvable", 404)
} on e is HttpError {
    IO::writeln(`${e.status} ${e.code} ${e.message}`)   // 404 500 introuvable
} on e is Exception {
    IO::writeln(`autre exception : ${e.message}`)
} on e {
    IO::writeln(`erreur inconnue : ${e}`)
}
```

### `raise` d'une chaîne

`raise` accepte aussi une simple chaîne (ou template string). Dans un handler générique `on e`, `e.message` rend alors cette chaîne ; `e.code` vaut `0` et `e.source` n'est pas renseigné.

```ocara
try {
    raise "simple chaîne"
} on e {
    IO::writeln(`générique : ${e.message}`)   // générique : simple chaîne
}
```

> Une chaîne levée n'a pas de classe : seul un handler générique `on e` l'attrape, jamais `on e is Exception`. Il en va de même pour un `raise` d'une expression dont la classe n'est pas connue statiquement (`mixed`…).

---

## Voir aussi

- [docs/EBNF.md](../EBNF.md) — §29 « Gestion des erreurs » (`try`/`on`, `raise`, classes d'exception)
- [UnitTest](UnitTest.md) — `assertRaises` et assertions sur `message`/`code`/`source`
