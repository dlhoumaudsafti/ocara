# Builtin AndroidUI

Spécificités de l'interface Android : bouton retour et autres événements
remontés à Ocara, barres système, orientation, plein écran, écran maintenu
allumé, vibration, partage, boîtes de dialogue et messages brefs.

```ocara
import ocara.AndroidUI
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
import ocara.AndroidUI
import ocara.System

function main(): void {
    if System::OS equal "android" {
        // Le bouton retour remonte ici au lieu de fermer l'app.
        AndroidUI::listen("back", nameless(data:string): void {
            if AndroidUI::confirm("Quitter", "Fermer l'application ?") {
                AndroidUI::close()
            }
        })
        AndroidUI::setKeepScreenOn(true)
        AndroidUI::toast("Prêt")
    }
}
```

## Événements

`AndroidUI::listen(event, callback)` appelle `callback` avec les données de
l'événement (texte JSON). Les callbacks s'exécutent sur un thread Ocara
dédié, l'un après l'autre — jamais sur le thread principal Android : un
callback lent ne gèle pas l'interface, et il peut appeler `confirm`.

| Événement | Données | Effet de `listen` |
|-----------|---------|-------------------|
| `back` | `null` | Le bouton retour est intercepté : il n'est plus traité par Android (l'app ne se ferme plus d'elle-même). |

Une exception non rattrapée dans un callback est signalée sur la sortie
d'erreur ; les événements suivants sont traités.

## Référence des méthodes

### `AndroidUI::listen(event, callback)` → `void`

Voir [Événements](#événements).

### `AndroidUI::setSystemBarsStyle(style)` → `void`

Couleur des icônes des barres système : `"light"` (icônes sombres, pour un
contenu clair) ou `"dark"` (icônes claires). L'app étant affichée sous les
barres (« edge-to-edge »), c'est le contenu de la page qui en fait la
couleur.

### `AndroidUI::setFullscreen(enabled)` → `void`

Masque (`true`) ou rétablit les barres système ; un balayage depuis le bord
les fait réapparaître un instant.

### `AndroidUI::setOrientation(orientation)` → `void`

`"portrait"`, `"landscape"` ou `"auto"` (suit la rotation de l'appareil).

### `AndroidUI::setKeepScreenOn(enabled)` → `void`

Empêche la mise en veille de l'écran tant que l'app est affichée.

### `AndroidUI::vibrate(ms)` → `void`

Vibration de `ms` millisecondes. `AndroidException` 103 sans vibreur.

### `AndroidUI::share(text, title)` → `void`

Ouvre la feuille de partage du système avec `text` (`title` : titre de la
feuille, `""` pour celui du système).

### `AndroidUI::alert(title, text)` → `void`

Boîte d'information ; bloque jusqu'à sa fermeture.

### `AndroidUI::confirm(title, text)` → `bool`

Boîte de confirmation : vrai si l'utilisateur valide, faux s'il annule ou
ferme la boîte.

### `AndroidUI::toast(text)` → `void`

Message bref en bas de l'écran.

### `AndroidUI::close()` → `void`

Ferme l'app (termine l'Activity).

## Codes d'erreur AndroidException

| Code | Cause |
|------|-------|
| 100 | Hors Android, ou aucune Activity au premier plan |
| 102 | `style`, `orientation` ou durée invalide |
| 103 | Pas de vibreur |
| 107 | `alert`/`confirm` appelés sur le thread principal Android |

## Voir aussi

- [AndroidPermissions](AndroidPermissions.md), [AndroidNotifications](AndroidNotifications.md)
- [Tauri](Tauri.md) — équivalent desktop (`alert`, `confirm`, `listen`)
