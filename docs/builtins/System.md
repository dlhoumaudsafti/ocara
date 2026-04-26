# System

Classe builtin `ocara.System` — interaction avec le système d'exploitation : exécution de commandes, variables d'environnement, processus et informations plateforme.

Toutes les méthodes sont **statiques**.

```ocara
import ocara.System
// ou
import ocara.*
```

---

## Constantes de classe

### `System::OS` → `string`

Système d'exploitation de la machine cible, déterminé à la compilation.

| Valeur | Plateforme |
|---|---|
| `"linux"` | Linux |
| `"macos"` | macOS |
| `"windows"` | Windows |

### `System::ARCH` → `string`

Architecture CPU.

| Valeur | Architecture |
|---|---|
| `"x86_64"` | Intel / AMD 64 bits |
| `"aarch64"` | ARM 64 bits |
| `"x86"` | Intel / AMD 32 bits |

```ocara
IO::writeln(`OS : ${System::OS}, ARCH : ${System::ARCH}`)
```

---

## Exécution de commandes

### `System::exec(cmd: string) → string`

Exécute `cmd` via le shell (`/bin/sh -c` sous Unix) et retourne la sortie standard complète sous forme de chaîne. La sortie d'erreur n'est pas capturée.

```ocara
scoped out:string = System::exec("uname -r")
IO::writeln(out)   // ex. "6.8.0-57-generic"
```

### `System::exec_code(cmd: string) → int`

Exécute `cmd` et retourne **uniquement le code de sortie**. Stdout et stderr ne sont pas capturés.

```ocara
scoped ok:int = System::exec_code("test -f /etc/hostname")
if ok == 0 {
    IO::writeln("fichier présent")
}
```

### `System::passthrough(cmd: string) → int`

Exécute `cmd` en héritant des descripteurs `stdin`, `stdout` et `stderr` du processus courant. L'affichage s'effectue directement dans le terminal. Retourne le code de sortie.

```ocara
scoped code:int = System::passthrough("ls -lh")
IO::writeln(`code : ${code}`)
```

---

## Contrôle du processus

### `System::exit(code: int) → void`

Termine immédiatement le processus avec le code de sortie `code`. N'exécute pas les destructeurs ni les fonctions `defer`.

```ocara
if erreur {
    IO::writeln("Erreur fatale")
    System::exit(1)
}
```

### `System::sleep(ms: int) → void`

Suspend l'exécution du processus pendant `ms` millisecondes.

```ocara
System::sleep(500)   // pause de 500 ms
```

### `System::pid() → int`

Retourne le PID (identifiant de processus) du processus courant.

```ocara
IO::writeln(`PID : ${System::pid()}`)
```

### `System::args() → string[]`

Retourne les arguments passés à la ligne de commande (équivalent de `argv` en C, `argv[0]` inclus).

```ocara
scoped argv:string[] = System::args()
```

---

## Répertoire de travail

### `System::cwd() → string`

Retourne le chemin absolu du répertoire de travail courant.

```ocara
IO::writeln(System::cwd())   // ex. "/home/alice/projet"
```

---

## Variables d'environnement

### `System::env(name: string) → string`

Retourne la valeur de la variable d'environnement `name`. Retourne `""` si elle est absente.

```ocara
scoped home:string = System::env("HOME")
scoped path:string = System::env("PATH")
```

### `System::set_env(name: string, value: string) → void`

Définit ou remplace la variable d'environnement `name` avec la valeur `value`. Visible par les processus enfants lancés après cet appel.

```ocara
System::set_env("APP_ENV", "production")
System::set_env("LOG_LEVEL", "debug")
```

---

## Exemples complets

### Détection de plateforme

```ocara
import ocara.System
import ocara.IO

function main(): int {
    if System::OS == "linux" {
        IO::writeln("Système Linux détecté")
    } elseif System::OS == "macos" {
        IO::writeln("Système macOS détecté")
    } else {
        IO::writeln(`Système : ${System::OS}`)
    }
    return 0
}
```

### Exécuter une commande et traiter la sortie

```ocara
import ocara.System
import ocara.String
import ocara.IO

function main(): int {
    scoped kernel:string = System::exec("uname -r")
    scoped trimmed:string = String::trim(kernel)
    IO::writeln(`Kernel : ${trimmed}`)
    return 0
}
```

### Script de déploiement

```ocara
import ocara.System
import ocara.IO

function main(): int {
    IO::writeln("Déploiement en cours…")

    scoped build:int = System::exec_code("cargo build --release")
    if build != 0 {
        IO::writeln("Échec du build")
        System::exit(1)
    }

    System::set_env("APP_ENV", "production")
    scoped deploy:int = System::passthrough("./scripts/deploy.sh")

    if deploy == 0 {
        IO::writeln("Déploiement réussi")
    } else {
        IO::writeln(`Déploiement échoué (code : ${deploy})`)
        System::exit(deploy)
    }

    return 0
}
```

---

## Symboles runtime

| Méthode Ocara | Symbole C runtime |
|---|---|
| `exec` | `System_exec` |
| `passthrough` | `System_passthrough` |
| `exec_code` | `System_exec_code` |
| `exit` | `System_exit` |
| `env` | `System_env` |
| `set_env` | `System_set_env` |
| `cwd` | `System_cwd` |
| `sleep` | `System_sleep` |
| `pid` | `System_pid` |
| `args` | `System_args` |
