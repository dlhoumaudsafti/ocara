# Builtin AndroidFiles

Répertoires de l'app, fichiers embarqués dans l'APK (`assets/`) et dossier
Téléchargements partagé. Les fichiers eux-mêmes se lisent et s'écrivent avec
[File](File.md) et [Directory](Directory.md).

```ocara
import ocara.AndroidFiles
```

## Disponibilité

Programmes compilés pour Android et lancés par l'app de `packaging/android`
(pont JNI `OcaraBridge`, classes Kotlin `com.ocara.bridge.Ocara*`) : voir
[docs/android.md](../android.md), section « Builtins natifs Android ». Sur
une autre cible, le programme compile et chaque appel lève
`AndroidException` 100 : une même source peut servir le desktop et Android
(`if System::OS equal "android" { ... }`).

Le répertoire de travail du programme est `filesDir()` : un chemin relatif
(`"data.json"`) y mène, dans `File` comme dans les builtins `Android*`.

## Exemple

```ocara
import ocara.AndroidFiles
import ocara.File

function main(): void {
    if not File::exists("config.json") {
        AndroidFiles::copyAsset("config.json", "config.json")
    }
    AndroidFiles::saveToDownloads("rapport.pdf", "rapport-ocara.pdf")
}
```

## Référence des méthodes

### `AndroidFiles::filesDir()` → `string`

Stockage interne privé de l'app (répertoire de travail du programme).

### `AndroidFiles::cacheDir()` → `string`

Cache privé, que le système peut vider.

### `AndroidFiles::externalFilesDir()` → `string`

Stockage externe propre à l'app (`Android/data/<package>/files`), lisible
par l'utilisateur sur ordinateur. `AndroidException` 103 s'il est absent.

### `AndroidFiles::listAssets(dir)` → `array<string>`

Noms des entrées du dossier `dir` des assets de l'APK (`""` : la racine).

### `AndroidFiles::readAsset(path)` → `string`

Contenu texte (UTF-8) d'un asset. `AndroidException` 104 s'il est absent.

### `AndroidFiles::copyAsset(path, dest)` → `void`

Copie un asset vers `dest` (dossiers créés au besoin).

### `AndroidFiles::saveToDownloads(source, name)` → `string`

Copie le fichier `source` dans Téléchargements sous le nom `name`, visible
de l'utilisateur et des autres apps. Rend l'URI `content://` (Android 10 et
plus, aucune permission) ou le chemin (Android 9 et moins, permission
`android.permission.WRITE_EXTERNAL_STORAGE`).

## Codes d'erreur AndroidException

| Code | Cause |
|------|-------|
| 100 | Hors Android, ou app qui ne démarre pas le programme par `OcaraBridge` |
| 101 | `saveToDownloads` sous Android 9 sans permission d'écriture |
| 103 | Stockage externe indisponible |
| 104 | Asset ou fichier introuvable, écriture impossible |
