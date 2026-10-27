# Builtin AndroidPermissions

Permissions d'exécution Android : vérifier et demander à l'utilisateur
l'accès à la caméra, au micro, aux notifications…

```ocara
import ocara.AndroidPermissions
```

## Disponibilité

Programmes compilés pour Android et lancés par l'app de `packaging/android`
(pont JNI `OcaraBridge`, classes Kotlin `com.ocara.bridge.Ocara*`) : voir
[docs/android.md](../android.md), section « Builtins natifs Android ». Sur
une autre cible, le programme compile et chaque appel lève
`AndroidException` 100 : une même source peut servir le desktop et Android
(`if System::OS equal "android" { ... }`).

Une permission doit **aussi** être déclarée dans `AndroidManifest.xml`
(`<uses-permission android:name="android.permission.CAMERA" />`) : sans
déclaration, Android la refuse sans rien demander.

## Exemple

```ocara
import ocara.AndroidPermissions
import ocara.AndroidCamera
import ocara.IO

function main(): void {
    if AndroidPermissions::request("camera") {
        AndroidCamera::takePhoto("photo.jpg", "back")
    } else {
        IO::writeln("caméra refusée")
    }
}
```

## Noms de permission

Nom court ou nom Android complet (`android.permission.CAMERA`).

| Nom court | Permission Android |
|-----------|--------------------|
| `camera` | `CAMERA` |
| `microphone` | `RECORD_AUDIO` |
| `notifications` | `POST_NOTIFICATIONS` (accordée d'office avant Android 13) |
| `location` | `ACCESS_FINE_LOCATION` |
| `coarse_location` | `ACCESS_COARSE_LOCATION` |
| `contacts` | `READ_CONTACTS` |
| `activity_recognition` | `ACTIVITY_RECOGNITION` (capteur `step_counter`) |

## Référence des méthodes

### `AndroidPermissions::isGranted(permission)` → `bool`

Vrai si la permission est accordée. Ne demande rien.

### `AndroidPermissions::request(permission)` → `bool`

Demande la permission si elle n'est pas déjà accordée et **attend la réponse
de l'utilisateur** (le thread Ocara est bloqué, pas l'interface Android).
Rend vrai si elle est accordée. `AndroidException` 107 si appelée depuis le
thread principal Android.

## Codes d'erreur AndroidException

| Code | Cause |
|------|-------|
| 100 | Hors Android, ou app qui ne démarre pas le programme par `OcaraBridge` |
| 102 | Nom de permission inconnu |
| 107 | `request` appelée sur le thread principal Android |

## Voir aussi

- [AndroidCamera](AndroidCamera.md), [AndroidMicrophone](AndroidMicrophone.md), [AndroidNotifications](AndroidNotifications.md)
