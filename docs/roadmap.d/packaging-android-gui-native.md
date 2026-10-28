# Builtin `ocara.AndroidUI` (interface Android native)

Statut : **pas commencé**, défini par l'analyse d'[`ocara.UI`](langage-builtin-ui-multiplateforme.md)
(2026-10-28), dont il est l'étape 7.

## Rôle

Les spécificités de l'interface Android native, celles qu'`ocara.UI` ne
standardise pas (décision de David : tout ce qui se standardise passe par
`ocara.UI`, le reste par `ocara.Tauri` sur desktop et `ocara.AndroidUI` sur
Android) :

- barres système (couleur, masquage), orientation, plein écran immersif ;
- écran maintenu allumé, retour haptique ;
- partage vers une autre app (`Intent.ACTION_SEND`) ;
- composants Material sans équivalent desktop (bouton flottant, panneau
  inférieur, tiroir de navigation).

Il s'utilise seul (`use AndroidUI()`, y compris dans l'app hybride de
`mini_project_hexa` pour régler barres système et orientation) ou depuis une
interface `ocara.UI` (`ui.android()`, question 3 de l'analyse).

## Piste technique retenue

L'app de `packaging/android` est déjà en Jetpack Compose (Material3,
navigation3). Le rendu natif d'`ocara.UI` sur Android est un moteur Compose
(`OcaraUI.kt`) qui interprète l'arbre d'un `.oc.ui`, piloté par le pont JNI
existant (`runtime_android/common`) ; `ocara.AndroidUI` partage ce crate
(`runtime_android/ui`) et cette classe Kotlin. Pistes écartées :
`ANativeActivity` (rendu bas niveau sans composants natifs) et un pont JNI
widget par widget (surface énorme, propriété des objets Java depuis Ocara).

Comme les builtins `ocara.Android*`, il est lié dans le `.so` seulement s'il
est importé, et lève `AndroidException` 100 hors Android.

## Complexité

Structurel une fois le moteur Compose d'`ocara.UI` en place (étape 3 de
l'analyse) ; chaque spécificité est ensuite un appel Kotlin de quelques
lignes.

## Fichiers clés

[langage-builtin-ui-multiplateforme](langage-builtin-ui-multiplateforme.md)
(analyse, §8), `runtime_android/common/` (appels Kotlin), `packaging/android/`
(app Compose), [packaging-android-native-features](packaging-android-native-features.md)
(builtins Android hors interface, même schéma de crates).
