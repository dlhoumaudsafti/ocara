# Builtin AndroidAudio

Lecture d'un fichier audio (MP3, AAC/M4A, OGG, WAV… : formats de
`MediaPlayer`), une lecture à la fois. Pour le son d'un jeu, voir
[SDL](SDL.md).

```ocara
import ocara.AndroidAudio
```

## Disponibilité

Programmes compilés pour Android et lancés par l'app de `packaging/android`
(pont JNI `OcaraBridge`, classes Kotlin `com.ocara.bridge.Ocara*`) : voir
[docs/android.md](../android.md), section « Builtins natifs Android ». Sur
une autre cible, le programme compile et chaque appel lève
`AndroidException` 100 : une même source peut servir le desktop et Android
(`if System::OS equal "android" { ... }`).

## Exemple

```ocara
import ocara.AndroidAudio
import ocara.Thread

function main(): void {
    AndroidAudio::setVolume(0.5)
    AndroidAudio::play("musique.mp3")
    while AndroidAudio::isPlaying() {
        Thread::sleep(200)
    }
}
```

## Référence des méthodes

### `AndroidAudio::play(path)` → `void`

Lit le fichier `path` depuis le début ; arrête la lecture précédente.

### `AndroidAudio::pause()` → `void`

Met en pause.

### `AndroidAudio::resume()` → `void`

Reprend après une pause.

### `AndroidAudio::stop()` → `void`

Arrête la lecture (sans effet s'il n'y en a pas).

### `AndroidAudio::isPlaying()` → `bool`

Vrai pendant la lecture (faux en pause et à la fin du fichier).

### `AndroidAudio::setVolume(volume)` → `void`

Volume de l'app, de `0.0` à `1.0` (défaut `1.0`), gardé pour les lectures
suivantes.

## Codes d'erreur AndroidException

| Code | Cause |
|------|-------|
| 100 | Hors Android, ou app qui ne démarre pas le programme par `OcaraBridge` |
| 102 | Volume hors de 0.0..1.0 |
| 104 | Fichier introuvable ou illisible |
| 107 | `pause`/`resume` sans lecture |
