# Builtin AndroidCamera

Photo JPEG sans aperçu affiché (Camera2) : la caméra s'ouvre, l'exposition
automatique converge (au plus 3 s), la photo est prise à la plus grande
résolution, orientée selon le capteur, puis la caméra est fermée.

```ocara
import ocara.AndroidCamera
```

## Disponibilité

Programmes compilés pour Android et lancés par l'app de `packaging/android`
(pont JNI `OcaraBridge`, classes Kotlin `com.ocara.bridge.Ocara*`) : voir
[docs/android.md](../android.md), section « Builtins natifs Android ». Sur
une autre cible, le programme compile et chaque appel lève
`AndroidException` 100 : une même source peut servir le desktop et Android
(`if System::OS equal "android" { ... }`).

Permission `camera` requise (`AndroidPermissions::request("camera")`,
`android.permission.CAMERA` dans le manifeste).

## Exemple

```ocara
import ocara.AndroidPermissions
import ocara.AndroidCamera
import ocara.File
import ocara.IO

function main(): void {
    if AndroidPermissions::request("camera") {
        AndroidCamera::takePhoto("photo.jpg", "back")
        IO::writeln(`${File::size("photo.jpg")} octets`)
    }
}
```

## Référence des méthodes

### `AndroidCamera::count()` → `int`

Nombre de caméras de l'appareil.

### `AndroidCamera::takePhoto(path, facing)` → `void`

Prend une photo et l'enregistre en JPEG dans `path`. `facing` :
`"back"` (dos, aussi pour `""`) ou `"front"` (face). Bloque jusqu'à la
photo, 10 s au plus.

## Codes d'erreur AndroidException

| Code | Cause |
|------|-------|
| 100 | Hors Android, ou app qui ne démarre pas le programme par `OcaraBridge` |
| 101 | Permission `camera` non accordée |
| 102 | `facing` autre que `"back"` ou `"front"` |
| 103 | Aucune caméra de ce côté, caméra en erreur ou déconnectée |
| 106 | Aucune photo en 10 s |
| 107 | Appelée sur le thread principal Android |
