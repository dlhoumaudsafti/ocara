# Builtin AndroidMicrophone

Enregistrement du micro en AAC (fichier `.m4a`, 44,1 kHz, 128 kbit/s), par
`MediaRecorder`. Un enregistrement à la fois.

```ocara
import ocara.AndroidMicrophone
```

## Disponibilité

Programmes compilés pour Android et lancés par l'app de `packaging/android`
(pont JNI `OcaraBridge`, classes Kotlin `com.ocara.bridge.Ocara*`) : voir
[docs/android.md](../android.md), section « Builtins natifs Android ». Sur
une autre cible, le programme compile et chaque appel lève
`AndroidException` 100 : une même source peut servir le desktop et Android
(`if System::OS equal "android" { ... }`).

Permission `microphone` requise (`AndroidPermissions::request("microphone")`,
`android.permission.RECORD_AUDIO` dans le manifeste).

## Exemple

```ocara
import ocara.AndroidPermissions
import ocara.AndroidMicrophone
import ocara.Thread

function main(): void {
    if AndroidPermissions::request("microphone") {
        AndroidMicrophone::start("note.m4a")
        Thread::sleep(5000)
        AndroidMicrophone::stop()
    }
}
```

## Référence des méthodes

### `AndroidMicrophone::start(path)` → `void`

Démarre l'enregistrement vers `path`.

### `AndroidMicrophone::stop()` → `void`

Arrête l'enregistrement et finalise le fichier. `AndroidException` 104 si
rien n'a été enregistré (arrêt immédiat).

### `AndroidMicrophone::isRecording()` → `bool`

Vrai entre `start` et `stop`.

## Codes d'erreur AndroidException

| Code | Cause |
|------|-------|
| 100 | Hors Android, ou app qui ne démarre pas le programme par `OcaraBridge` |
| 101 | Permission `microphone` non accordée |
| 103 | Micro indisponible (déjà utilisé par une autre app…) |
| 104 | Enregistrement vide ou invalide |
| 107 | `start` pendant un enregistrement, `stop` sans enregistrement |

## Voir aussi

- [AndroidAudio](AndroidAudio.md) — relire l'enregistrement
