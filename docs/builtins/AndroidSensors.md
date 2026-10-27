# Builtin AndroidSensors

Relevés des capteurs de l'appareil (accéléromètre, gyroscope, lumière…),
par l'API NDK `ASensorManager`, sans Java.

```ocara
import ocara.AndroidSensors
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
import ocara.AndroidSensors
import ocara.IO

function main(): void {
    for name in AndroidSensors::list() {
        const values:array<float> = AndroidSensors::read(name)
        IO::writeln(`${name} : ${values.join(", ")}`)
    }
}
```

## Capteurs

| Nom | Valeurs | Unité |
|-----|---------|-------|
| `accelerometer` | x, y, z | m/s² |
| `magnetic_field` | x, y, z | µT |
| `gyroscope` | x, y, z | rad/s |
| `light` | 1 | lx |
| `pressure` | 1 | hPa |
| `proximity` | 1 | cm |
| `gravity` | x, y, z | m/s² |
| `linear_acceleration` | x, y, z | m/s² |
| `rotation_vector` | x, y, z, w | quaternion |
| `relative_humidity` | 1 | % |
| `ambient_temperature` | 1 | °C |
| `step_counter` | 1 | pas depuis le démarrage (permission `activity_recognition`) |

## Référence des méthodes

### `AndroidSensors::isAvailable(sensor)` → `bool`

Vrai si l'appareil a ce capteur. `AndroidException` 102 pour un nom inconnu.

### `AndroidSensors::list()` → `array<string>`

Noms des capteurs du tableau ci-dessus présents sur l'appareil.

### `AndroidSensors::read(sensor)` → `array<float>`

Un relevé : le capteur est activé (50 Hz) le temps du premier événement, au
plus 2 s. `AndroidException` 103 si l'appareil n'a pas ce capteur, 106 si
aucun relevé n'arrive (un capteur « à changement », comme `proximity`, peut
ne rien émettre tant que rien ne change), 101 pour `step_counter` sans la
permission.

## Codes d'erreur AndroidException

| Code | Cause |
|------|-------|
| 100 | Hors Android, ou app qui ne démarre pas le programme par `OcaraBridge` |
| 101 | `step_counter` sans la permission `activity_recognition` |
| 102 | Nom de capteur inconnu |
| 103 | Capteur absent de l'appareil |
| 105 | Capteur impossible à activer |
| 106 | Aucun relevé en 2 s |
