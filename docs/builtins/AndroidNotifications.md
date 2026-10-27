# Builtin AndroidNotifications

Notifications système Android, dans un canal « ocara » créé au premier envoi
(nommé comme l'app). Toucher la notification ouvre l'app.

```ocara
import ocara.AndroidNotifications
```

## Disponibilité

Programmes compilés pour Android et lancés par l'app de `packaging/android`
(pont JNI `OcaraBridge`, classes Kotlin `com.ocara.bridge.Ocara*`) : voir
[docs/android.md](../android.md), section « Builtins natifs Android ». Sur
une autre cible, le programme compile et chaque appel lève
`AndroidException` 100 : une même source peut servir le desktop et Android
(`if System::OS equal "android" { ... }`).

Depuis Android 13, la permission `notifications` est requise :
`AndroidPermissions::request("notifications")`, et
`android.permission.POST_NOTIFICATIONS` déclarée dans le manifeste.

## Exemple

```ocara
import ocara.AndroidPermissions
import ocara.AndroidNotifications

function main(): void {
    if AndroidPermissions::request("notifications") {
        const id:int = AndroidNotifications::show("Ocara", "Sauvegarde terminée")
        AndroidNotifications::cancel(id)
    }
}
```

## Référence des méthodes

### `AndroidNotifications::show(title, text)` → `int`

Affiche une notification et rend son identifiant. `AndroidException` 101 si
la permission manque.

### `AndroidNotifications::cancel(id)` → `void`

Retire la notification `id` (sans effet si elle n'existe plus).

### `AndroidNotifications::areEnabled()` → `bool`

Faux si l'utilisateur a désactivé les notifications de l'app.

## Codes d'erreur AndroidException

| Code | Cause |
|------|-------|
| 100 | Hors Android, ou app qui ne démarre pas le programme par `OcaraBridge` |
| 101 | Permission `notifications` non accordée |

## Voir aussi

- [AndroidPermissions](AndroidPermissions.md)
