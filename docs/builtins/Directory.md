# ocara.Directory

Classe builtin statique pour les opérations sur répertoires.

## Import

```ocara
import ocara.Directory
```

## Création et suppression

### `Directory::create(path: string) → void`

Crée un répertoire (le parent doit exister).

```ocara
Directory::create("/tmp/data")
```

**Erreur** : `fail` si le parent n'existe pas ou si le répertoire existe déjà.

### `Directory::create_recursive(path: string) → void`

Crée un répertoire et tous ses parents (équivalent `mkdir -p`).

```ocara
Directory::create_recursive("/tmp/project/src/utils")
```

**Erreur** : `fail` en cas d'erreur de permissions.

### `Directory::remove(path: string) → void`

Supprime un répertoire vide.

```ocara
Directory::remove("/tmp/empty")
```

**Erreur** : `fail` si le répertoire n'est pas vide ou n'existe pas.

### `Directory::remove_recursive(path: string) → void`

Supprime un répertoire et tout son contenu (équivalent `rm -rf`).

```ocara
Directory::remove_recursive("/tmp/old_project")
```

**Erreur** : `fail` en cas d'erreur de permissions.

## Listing

### `Directory::list(path: string) → string[]`

Liste tous les fichiers et répertoires d'un répertoire.

```ocara
var entries:string[] = Directory::list("/tmp")

for entry in entries {
    IO::writeln(entry)
}
```

**Erreur** : `fail` si le répertoire n'existe pas ou n'est pas accessible.

### `Directory::list_files(path: string) → string[]`

Liste uniquement les fichiers d'un répertoire.

```ocara
var files:string[] = Directory::list_files("/tmp")

for file in files {
    IO::writeln(`Fichier: ${file}`)
}
```

**Erreur** : `fail` si le répertoire n'existe pas ou n'est pas accessible.

### `Directory::list_dirs(path: string) → string[]`

Liste uniquement les sous-répertoires d'un répertoire.

```ocara
var dirs:string[] = Directory::list_dirs("/tmp")

for dir in dirs {
    IO::writeln(`Répertoire: ${dir}`)
}
```

**Erreur** : `fail` si le répertoire n'existe pas ou n'est pas accessible.

## Métadonnées

### `Directory::exists(path: string) → bool`

Teste si un répertoire existe.

```ocara
if !Directory::exists("/var/log/myapp") {
    Directory::create("/var/log/myapp")
}
```

### `Directory::count(path: string) → int`

Compte le nombre d'entrées dans un répertoire.

```ocara
var count:int = Directory::count("/tmp")
IO::writeln(`Nombre d'entrées: ${count}`)
```

**Erreur** : `fail` si le répertoire n'existe pas ou n'est pas accessible.

### `Directory::infos(path: string) → map<string, mixed>`

Retourne les métadonnées d'un répertoire.

```ocara
var infos:map<string, mixed> = Directory::infos("/tmp/data")

IO::writeln(`Modifié: ${infos["modified"]}`)
IO::writeln(`Nombre d'entrées: ${infos["count"]}`)
```

**Clés du map** :
- `modified` : `int` - Timestamp de dernière modification (Unix epoch)
- `created` : `int` - Timestamp de création (Unix epoch)
- `is_dir` : `bool` - Toujours `true` pour un répertoire
- `count` : `int` - Nombre d'entrées dans le répertoire

**Erreur** : `fail` si le répertoire n'existe pas.

## Opérations

### `Directory::copy(src: string, dst: string) → void`

Copie un répertoire et tout son contenu (récursif).

```ocara
Directory::copy("/tmp/project", "/tmp/backup")
```

**Erreur** : `fail` si la source n'existe pas ou si l'écriture échoue.

### `Directory::move(src: string, dst: string) → void`

Déplace ou renomme un répertoire.

```ocara
Directory::move("/tmp/old_name", "/tmp/new_name")
```

**Erreur** : `fail` si la source n'existe pas ou si l'opération échoue.

## Gestion d'erreurs

Toutes les méthodes qui peuvent échouer lèvent une `DirectoryException`. Utilisez `try`/`on` pour gérer les erreurs :

```ocara
try {
    Directory::create_recursive("/tmp/project/src")
    IO::writeln("Répertoire créé")
} on e is DirectoryException {
    IO::writeln(`Erreur: ${e.message}`)
    IO::writeln(`Code d'erreur: ${e.code}`)
}
```

Pour plus de détails sur les codes d'erreur spécifiques, consultez la section [Codes d'erreur des exceptions](#codes-derreur-des-exceptions) ci-dessous.

## Exemples complets

### Créer une arborescence de projet

```ocara
import ocara.Directory
import ocara.File
import ocara.IO

function init_project(root:string): void {
    // Créer l'arborescence
    Directory::create_recursive(`${root}/src`)
    Directory::create_recursive(`${root}/tests`)
    Directory::create_recursive(`${root}/docs`)
    Directory::create_recursive(`${root}/build`)
    
    // Créer des fichiers par défaut
    File::write(`${root}/README.md`, "# Mon Projet")
    File::write(`${root}/src/main.oc`, "function main(): int { return 0 }")
    
    IO::writeln("Projet initialisé")
}

function main(): int {
    try {
        init_project("/tmp/myproject")
    } on e is DirectoryException {
        IO::writeln(`Erreur répertoire: ${e.message}`)
    }
    return 0
}
```

### Lister récursivement tous les fichiers

```ocara
import ocara.Directory
import ocara.File
import ocara.IO

function list_recursive(path:string, prefix:string): void {
    var entries:string[] = Directory::list(path)
    
    for entry in entries {
        var full_path:string = `${path}/${entry}`
        IO::writeln(`${prefix}${entry}`)
        
        // Si c'est un répertoire, lister récursivement
        if Directory::exists(full_path) {
            list_recursive(full_path, `${prefix}  `)
        }
    }
}

function main(): int {
    try {
        list_recursive("/tmp/project", "")
    } on e is DirectoryException {
        IO::writeln(`Erreur répertoire: ${e.message}`)
    }
    return 0
}
```

### Compter les fichiers par extension

```ocara
import ocara.Directory
import ocara.File
import ocara.Map
import ocara.IO

function count_by_extension(path:string): map<string, int> {
    var counts:map<string, int> = use map<string, int>()
    var files:string[] = Directory::list_files(path)
    
    for file in files {
        var ext:string = File::extension(`${path}/${file}`)
        if ext == "" {
            ext = "(sans extension)"
        }
        
        if Map::has(counts, ext) {
            var current:int = counts[ext]
            counts[ext] = current + 1
        } else {
            counts[ext] = 1
        }
    }
    
    return counts
}

function main(): int {
    try {
        var counts:map<string, int> = count_by_extension("/tmp/data")
        
        for ext => count in counts {
            IO::writeln(`${ext}: ${count} fichier(s)`)
        }
    } on e is DirectoryException {
        IO::writeln(`Erreur répertoire: ${e.message}`)
    }
    return 0
}
```

### Sauvegarde avec rotation

```ocara
import ocara.Directory
import ocara.IO

function rotate_backups(backup_dir:string, max_backups:int): void {
    if !Directory::exists(backup_dir) {
        Directory::create(backup_dir)
        return
    }
    
    var dirs:string[] = Directory::list_dirs(backup_dir)
    var count:int = Array::length(dirs)
    
    // Si on dépasse le max, supprimer les plus anciens
    if count >= max_backups {
        // Tri par nom (backup_1, backup_2, ...)
        Array::sort(dirs)
        
        // Supprimer le plus ancien
        var oldest:string = `${backup_dir}/${dirs[0]}`
        Directory::remove_recursive(oldest)
        IO::writeln(`Suppression: ${oldest}`)
    }
    
    // Créer un nouveau backup
    var new_backup:string = `${backup_dir}/backup_${count + 1}`
    Directory::create(new_backup)
    IO::writeln(`Nouveau backup: ${new_backup}`)
}

function main(): int {
    try {
        rotate_backups("/var/backups/myapp", 5)
    } on e is DirectoryException {
        IO::writeln(`Erreur répertoire: ${e.message}`)
    }
    return 0
}
```

### Nettoyer les fichiers temporaires

```ocara
import ocara.Directory
import ocara.File
import ocara.IO

function clean_temp_files(path:string): int {
    var count:int = 0
    var files:string[] = Directory::list_files(path)
    
    for file in files {
        var ext:string = File::extension(file)
        
        // Supprimer .tmp, .temp, .bak
        if ext == "tmp" || ext == "temp" || ext == "bak" {
            var full_path:string = `${path}/${file}`
            File::remove(full_path)
            IO::writeln(`Supprimé: ${file}`)
            count = count + 1
        }
    }
    
    return count
}

function main(): int {
    try {
        var count:int = clean_temp_files("/tmp")
        IO::writeln(`${count} fichier(s) supprimé(s)`)
    } on e is DirectoryException {
        IO::writeln(`Erreur répertoire: ${e.message}`)
    }
    return 0
}
```

## Opérations combinées File/Directory

### Copier tous les fichiers d'un type

```ocara
import ocara.Directory
import ocara.File
import ocara.IO

function copy_by_extension(src:string, dst:string, ext:string): int {
    var count:int = 0
    var files:string[] = Directory::list_files(src)
    
    if !Directory::exists(dst) {
        Directory::create(dst)
    }
    
    for file in files {
        if File::extension(file) == ext {
            File::copy(`${src}/${file}`, `${dst}/${file}`)
            count = count + 1
        }
    }
    
    return count
}

function main(): int {
    try {
        var count:int = copy_by_extension("/tmp/source", "/tmp/dest", "txt")
        IO::writeln(`${count} fichier(s) .txt copié(s)`)
    } on e is DirectoryException {
        IO::writeln(`Erreur répertoire: ${e.message}`)
    }
    return 0
}
```

## Codes d'erreur des exceptions

Toutes les opérations `Directory` susceptibles d'échouer lèvent une `DirectoryException` avec des codes d'erreur spécifiques permettant une gestion précise des erreurs.

### Référence des codes d'erreur

| Code | Nom | Opération | Description |
|------|------|-----------|-------------|
| 101 | `CREATE` | `Directory::create()` | Échec de création du répertoire (parent inexistant, permission refusée, existe déjà, etc.) |
| 102 | `CREATE_RECURSIVE` | `Directory::create_recursive()` | Échec de création récursive (permission refusée, chemin invalide, etc.) |
| 103 | `REMOVE` | `Directory::remove()` | Échec de suppression du répertoire (introuvable, non vide, permission refusée, etc.) |
| 104 | `REMOVE_RECURSIVE` | `Directory::remove_recursive()` | Échec de suppression récursive (permission refusée, répertoire utilisé, etc.) |
| 105 | `LIST` | `Directory::list()` | Échec de listage du contenu (répertoire introuvable, permission refusée, etc.) |
| 106 | `LIST_FILES` | `Directory::list_files()` | Échec de listage des fichiers (répertoire introuvable, permission refusée, etc.) |
| 107 | `LIST_DIRS` | `Directory::list_dirs()` | Échec de listage des sous-répertoires (répertoire introuvable, permission refusée, etc.) |
| 108 | `COUNT` | `Directory::count()` | Échec de comptage des entrées (répertoire introuvable, permission refusée, etc.) |
| 109 | `COPY` | `Directory::copy()` | Échec de copie du répertoire (source introuvable, erreur destination, disque plein, etc.) |
| 110 | `MOVE` | `Directory::move()` | Échec de déplacement/renommage (source introuvable, destination existe, erreur cross-device, etc.) |
| 111 | `INFOS` | `Directory::infos()` | Échec de lecture des métadonnées (répertoire introuvable, permission refusée, etc.) |

### Exemples de gestion d'erreurs

#### Gestion générique des erreurs

Capturer toute `DirectoryException` et afficher le message d'erreur :

```ocara
import ocara.Directory
import ocara.IO

function main(): int {
    try {
        var entries:string[] = Directory::list("/tmp/data")
        for entry in entries {
            IO::writeln(entry)
        }
    } on e is DirectoryException {
        IO::writeln(`Erreur répertoire: ${e.message}`)
        IO::writeln(`Code d'erreur: ${e.code}`)
        IO::writeln(`Source: ${e.source}`)
    }
    return 0
}
```

#### Gestion spécifique par code d'erreur

Gérer différentes opérations répertoire avec des codes d'erreur spécifiques :

```ocara
import ocara.Directory
import ocara.IO

function safe_create(path:string): void {
    try {
        Directory::create(path)
        IO::writeln("Répertoire créé avec succès")
    } on e is DirectoryException {
        if e.code == 101 {
            IO::writeln(`Erreur de création: Vérifiez que le répertoire parent existe`)
        } else {
            IO::writeln(`Erreur répertoire inattendue: ${e.message}`)
        }
    }
}

function main(): int {
    safe_create("/tmp/test/subdir")
    return 0
}
```

#### Distinguer les erreurs de création et de listage

```ocara
import ocara.Directory
import ocara.IO

function setup_and_list(path:string): void {
    try {
        Directory::create_recursive(path)
        var entries:string[] = Directory::list(path)
        IO::writeln(`Répertoire créé avec ${Array::length(entries)} entrées`)
    } on e is DirectoryException {
        if e.code == 102 {
            IO::writeln(`Impossible de créer le répertoire '${path}'`)
        } else if e.code == 105 {
            IO::writeln(`Impossible de lister le répertoire '${path}'`)
        } else {
            IO::writeln(`Erreur opération répertoire [${e.code}]: ${e.message}`)
        }
    }
}
```

#### Gérer plusieurs opérations

```ocara
import ocara.Directory
import ocara.IO

function manage_directory(path:string): void {
    try {
        if !Directory::exists(path) {
            IO::writeln("Le répertoire n'existe pas")
            return
        }
        
        var count:int = Directory::count(path)
        var files:string[] = Directory::list_files(path)
        var dirs:string[] = Directory::list_dirs(path)
        
        IO::writeln(`Total: ${count} entrées`)
        IO::writeln(`Fichiers: ${Array::length(files)}`)
        IO::writeln(`Sous-répertoires: ${Array::length(dirs)}`)
        
    } on e is DirectoryException {
        if e.code == 108 {
            IO::writeln("Échec du comptage des entrées")
        } else if e.code == 106 {
            IO::writeln("Échec du listage des fichiers")
        } else if e.code == 107 {
            IO::writeln("Échec du listage des sous-répertoires")
        } else {
            IO::writeln(`Erreur répertoire [${e.code}]: ${e.message}`)
        }
    }
}
```

#### Copie sécurisée avec gestion d'erreur

```ocara
import ocara.Directory
import ocara.IO

function safe_copy(src:string, dst:string): void {
    try {
        if !Directory::exists(src) {
            IO::writeln("Le répertoire source n'existe pas")
            return
        }
        
        if Directory::exists(dst) {
            IO::writeln("Le répertoire destination existe déjà")
            return
        }
        
        Directory::copy(src, dst)
        IO::writeln("Répertoire copié avec succès")
        
    } on e is DirectoryException {
        if e.code == 109 {
            IO::writeln(`Échec de copie de '${src}' vers '${dst}': ${e.message}`)
        } else {
            IO::writeln(`Erreur répertoire [${e.code}]: ${e.message}`)
        }
    }
}
```

### Format des messages d'exception

Tous les messages d'exception sont en anglais et incluent :
- L'opération qui a échoué
- Le(s) chemin(s) de répertoire concerné(s)
- L'erreur système sous-jacente

Exemples :
- `Failed to create directory '/tmp/test': Permission denied (os error 13)`
- `Failed to list directory '/var/logs': No such file or directory (os error 2)`
- `Failed to copy directory from '/src' to '/dst': File exists (os error 17)`

## Notes

- Les noms retournés par `list()`, `list_files()` et `list_dirs()` sont **relatifs** au répertoire listé
- Pour obtenir le chemin complet, concaténer le chemin du répertoire : `` `${path}/${entry}` ``
- Les entrées `.` et `..` ne sont **pas** incluses dans les listings
- `Directory::remove_recursive()` est **dangereux** - utilisez avec précaution
- Pour les fichiers, voir [File](File.md)
