# ocara.File

Classe builtin statique pour les opérations sur fichiers.

## Import

```ocara
import ocara.File
```

## Lecture

### `File::read(path: string) → string`

Lit le contenu d'un fichier en UTF-8.

```ocara
var content:string = File::read("/tmp/data.txt")
IO::writeln(content)
```

**Erreur** : `fail` si le fichier n'existe pas ou n'est pas lisible.

### `File::readBytes(path: string) → int[]`

Lit le contenu d'un fichier en binaire (array d'octets).

```ocara
var bytes:int[] = File::readBytes("/tmp/image.png")
IO::writeln(`Taille: ${Array::length(bytes)} octets`)
```

**Erreur** : `fail` si le fichier n'existe pas ou n'est pas lisible.

## Écriture

### `File::write(path: string, content: string) → void`

Écrit du contenu texte dans un fichier (écrase si existe).

```ocara
File::write("/tmp/output.txt", "Hello, World!")
```

**Erreur** : `fail` en cas d'erreur d'écriture (permissions, disque plein, etc.).

### `File::writeBytes(path: string, data: int[]) → void`

Écrit des données binaires dans un fichier.

```ocara
var data:int[] = [0x89, 0x50, 0x4E, 0x47]  // Signature PNG
File::writeBytes("/tmp/test.png", data)
```

**Erreur** : `fail` en cas d'erreur d'écriture.

### `File::append(path: string, content: string) → void`

Ajoute du contenu à la fin d'un fichier (crée si n'existe pas).

```ocara
File::append("/var/log/app.log", `[${DateTime::now()}] Nouvelle entrée\n`)
```

**Erreur** : `fail` en cas d'erreur d'écriture.

## Métadonnées

### `File::exists(path: string) → bool`

Teste si un fichier existe.

```ocara
if File::exists("/tmp/config.json") {
    var config:string = File::read("/tmp/config.json")
}
```

### `File::size(path: string) → int`

Retourne la taille d'un fichier en octets.

```ocara
var size:int = File::size("/tmp/data.bin")
IO::writeln(`Taille: ${size} octets`)
```

**Erreur** : `fail` si le fichier n'existe pas.

### `File::extension(path: string) → string`

Retourne l'extension du fichier sans le point.

```ocara
var ext:string = File::extension("/tmp/document.pdf")  // "pdf"
var ext2:string = File::extension("/tmp/README")       // ""
```

### `File::infos(path: string) → map<string, mixed>`

Retourne les métadonnées complètes d'un fichier.

```ocara
var infos:map<string, mixed> = File::infos("/tmp/data.txt")

IO::writeln(`Taille: ${infos["size"]} octets`)
IO::writeln(`Modifié: ${infos["modified"]}`)  // Timestamp Unix
IO::writeln(`Extension: ${infos["extension"]}`)
```

**Clés du map** :
- `size` : `int` - Taille en octets
- `modified` : `int` - Timestamp de dernière modification (Unix epoch)
- `created` : `int` - Timestamp de création (Unix epoch)
- `is_file` : `bool` - Toujours `true` pour un fichier
- `is_dir` : `bool` - Toujours `false` pour un fichier
- `extension` : `string` - Extension sans le point

**Erreur** : `fail` si le fichier n'existe pas.

## Opérations

### `File::remove(path: string) → void`

Supprime un fichier.

```ocara
File::remove("/tmp/temp.txt")
```

**Erreur** : `fail` si le fichier n'existe pas ou ne peut pas être supprimé.

### `File::copy(src: string, dst: string) → void`

Copie un fichier.

```ocara
File::copy("/tmp/original.txt", "/tmp/copie.txt")
```

**Erreur** : `fail` si la source n'existe pas ou si l'écriture échoue.

### `File::move(src: string, dst: string) → void`

Déplace ou renomme un fichier.

```ocara
File::move("/tmp/old.txt", "/tmp/new.txt")
```

**Erreur** : `fail` si la source n'existe pas ou si l'opération échoue.

## Gestion d'erreurs

Toutes les méthodes qui peuvent échouer lèvent une `FileException`. Utilisez `try`/`on` pour gérer les erreurs :

```ocara
try {
    var content:string = File::read("/tmp/config.json")
    IO::writeln("Configuration chargée")
} on e is FileException {
    IO::writeln(`Error: ${e.message}`)
    IO::writeln(`Error code: ${e.code}`)
}
```

Pour plus de détails sur les codes d'erreur spécifiques, consultez la section [Exception Error Codes](#exception-error-codes) ci-dessous.

## Exemples complets

### Lire et traiter un fichier texte

```ocara
import ocara.File
import ocara.String
import ocara.IO

function main(): int {
    try {
        var content:string = File::read("/tmp/data.txt")
        var lines:string[] = String::split(content, "\n")
        
        IO::writeln(`Number of lines: ${Array::length(lines)}`)
        
        for line in lines {
            IO::writeln(line)
        }
    } on e is FileException {
        IO::writeln(`Read error [${e.code}]: ${e.message}`)
    }
    
    return 0
}
```

### Copier un fichier avec vérification

```ocara
import ocara.File
import ocara.IO

function copy_if_newer(src:string, dst:string): void {
    if !File::exists(src) {
        IO::writeln("Source file does not exist")
        return
    }
    
    if File::exists(dst) {
        var src_info:map<string, mixed> = File::infos(src)
        var dst_info:map<string, mixed> = File::infos(dst)
        
        if src_info["modified"] <= dst_info["modified"] {
            IO::writeln("Destination already up to date")
            return
        }
    }
    
    File::copy(src, dst)
    IO::writeln("File copied successfully")
}

function main(): int {
    try {
        copy_if_newer("/tmp/source.txt", "/tmp/dest.txt")
    } on e is FileException {
        IO::writeln(`File operation error [${e.code}]: ${e.message}`)
    }
    return 0
}
```

### Logger dans un fichier

```ocara
import ocara.File
import ocara.DateTime
import ocara.IO

function log(message:string): void {
    var timestamp:string = DateTime::format(DateTime::now(), "%Y-%m-%d %H:%M:%S")
    var entry:string = `[${timestamp}] ${message}\n`
    File::append("/var/log/myapp.log", entry)
}

function main(): int {
    try {
        log("Application started")
        // ... application code ...
        log("Processing completed")
    } on e is FileException {
        IO::writeln(`Logging error [${e.code}]: ${e.message}`)
    }
    return 0
}
```

### Traiter des fichiers binaires

```ocara
import ocara.File
import ocara.IO

function main(): int {
    try {
        // Read an image
        var image:int[] = File::readBytes("/tmp/input.png")
        
        IO::writeln(`Image loaded: ${Array::length(image)} bytes`)
        
        // Save a copy
        File::writeBytes("/tmp/output.png", image)
        
        IO::writeln("Copy created successfully")
    } on e is FileException {
        IO::writeln(`Binary file error [${e.code}]: ${e.message}`)
    }
    return 0
}
```

## Codes d'erreur des exceptions

Toutes les opérations `File` susceptibles d'échouer lèvent une `FileException` avec des codes d'erreur spécifiques permettant une gestion précise des erreurs.

### Référence des codes d'erreur

| Code | Nom | Opération | Description |
|------|------|-----------|-------------|
| 101 | `READ` | `File::read()` | Échec de lecture du fichier texte (fichier introuvable, permission refusée, UTF-8 invalide, etc.) |
| 102 | `READ_BYTES` | `File::readBytes()` | Échec de lecture du fichier binaire (fichier introuvable, permission refusée, erreur I/O, etc.) |
| 103 | `WRITE` | `File::write()` | Échec d'écriture du fichier texte (permission refusée, disque plein, chemin invalide, etc.) |
| 104 | `WRITE_BYTES` | `File::writeBytes()` | Échec d'écriture du fichier binaire (permission refusée, disque plein, chemin invalide, etc.) |
| 105 | `APPEND` | `File::append()` | Échec d'ajout au fichier (permission refusée, erreur I/O, etc.) |
| 106 | `SIZE` | `File::size()` | Échec de lecture de la taille du fichier (fichier introuvable, permission refusée, etc.) |
| 107 | `REMOVE` | `File::remove()` | Échec de suppression du fichier (fichier introuvable, permission refusée, fichier utilisé, etc.) |
| 108 | `COPY` | `File::copy()` | Échec de copie du fichier (source introuvable, erreur destination, disque plein, etc.) |
| 109 | `MOVE` | `File::move()` | Échec de déplacement/renommage du fichier (source introuvable, destination existe, erreur cross-device, etc.) |
| 110 | `INFOS` | `File::infos()` | Échec de lecture des métadonnées (fichier introuvable, permission refusée, etc.) |

### Exemples de gestion d'erreurs

#### Gestion générique des erreurs

Capturer toute `FileException` et afficher le message d'erreur :

```ocara
import ocara.File
import ocara.IO

function main(): int {
    try {
        var content:string = File::read("/tmp/data.txt")
        IO::writeln(content)
    } on e is FileException {
        IO::writeln(`Erreur fichier: ${e.message}`)
        IO::writeln(`Code d'erreur: ${e.code}`)
        IO::writeln(`Source: ${e.source}`)
    }
    return 0
}
```

#### Gestion spécifique par code d'erreur

Gérer différentes opérations fichier avec des codes d'erreur spécifiques :

```ocara
import ocara.File
import ocara.IO

function safe_read(path:string): string {
    try {
        return File::read(path)
    } on e is FileException {
        if e.code == 101 {
            IO::writeln(`Erreur de lecture: Fichier introuvable ou illisible`)
        } else {
            IO::writeln(`Erreur fichier inattendue: ${e.message}`)
        }
        return ""
    }
}

function main(): int {
    var data:string = safe_read("/tmp/config.txt")
    if data != "" {
        IO::writeln(`Données: ${data}`)
    }
    return 0
}
```

#### Distinguer les erreurs de lecture et d'écriture

```ocara
import ocara.File
import ocara.IO

function copy_with_error_handling(src:string, dst:string): void {
    try {
        var content:string = File::read(src)
        File::write(dst, content)
        IO::writeln("Fichier copié avec succès")
    } on e is FileException {
        if e.code == 101 {
            IO::writeln(`Erreur fichier source: Impossible de lire '${src}'`)
        } else if e.code == 103 {
            IO::writeln(`Erreur fichier destination: Impossible d'écrire '${dst}'`)
        } else {
            IO::writeln(`Erreur opération fichier [${e.code}]: ${e.message}`)
        }
    }
}
```

#### Gérer plusieurs opérations

```ocara
import ocara.File
import ocara.IO

function process_file(path:string): void {
    try {
        if !File::exists(path) {
            IO::writeln("Le fichier n'existe pas")
            return
        }
        
        var size:int = File::size(path)
        var content:string = File::read(path)
        var infos:map<string, mixed> = File::infos(path)
        
        IO::writeln(`Taille: ${size} octets`)
        IO::writeln(`Contenu: ${content}`)
        IO::writeln(`Modifié: ${infos["modified"]}`)
        
    } on e is FileException {
        if e.code == 106 {
            IO::writeln("Échec de lecture de la taille du fichier")
        } else if e.code == 101 {
            IO::writeln("Échec de lecture du contenu du fichier")
        } else if e.code == 110 {
            IO::writeln("Échec de lecture des métadonnées du fichier")
        } else {
            IO::writeln(`Erreur fichier [${e.code}]: ${e.message}`)
        }
    }
}
```

### Format des messages d'exception

Tous les messages d'exception sont en anglais et incluent :
- L'opération qui a échoué
- Le(s) chemin(s) de fichier concerné(s)
- L'erreur système sous-jacente

Exemples :
- `Failed to read file '/tmp/data.txt': No such file or directory (os error 2)`
- `Failed to write file '/tmp/output.txt': Permission denied (os error 13)`
- `Failed to copy file from '/a.txt' to '/b.txt': Disk quota exceeded (os error 122)`

## Notes

- Les chemins peuvent être absolus (`/tmp/file.txt`) ou relatifs (`./data/config.json`)
- Les chemins relatifs sont résolus depuis le répertoire courant du processus
- `File::write()` crée le fichier s'il n'existe pas
- `File::append()` crée le fichier s'il n'existe pas
- Les timestamps sont en secondes depuis l'epoch Unix (1er janvier 1970)
- Pour les répertoires, voir [Directory](Directory.md)
