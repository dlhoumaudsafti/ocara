# Interface native, étape 2 : builtin `ocara.AndroidUI`

Statut : **à faire, deuxième étape** du chantier interface native
([`ocara.UI`](langage-builtin-ui-multiplateforme.md), §10). Indépendante de
l'étape 1 ([finaliser `ocara.Tauri`](builtins-tauri.md)).

## Rôle

Les spécificités de l'interface Android native, celles qu'`ocara.UI` ne
standardise pas (décision de David : tout ce qui se standardise passe par
`ocara.UI`, le reste par `ocara.Tauri` sur desktop et `ocara.AndroidUI` sur
Android). Il s'utilise seul (`use AndroidUI()`) ou, plus tard, depuis une
interface `ocara.UI` (`ui.android()`).

## À faire dans cette étape (app hybride existante)

Tout ce qui fonctionne sans le moteur de rendu Compose, donc déjà dans l'app
de `packaging/android` (WebView + `HTTPServer`, `mini_project_hexa`) :

1. **Canal d'événements Kotlin → Ocara** — le préalable de tout le reste.
   Aujourd'hui Ocara sait appeler Kotlin (`runtime_android/common`), mais le
   pont JNI ne sait que démarrer `main`. Il faut une fonction native
   (`OcaraBridge.nativeEvent(action, id, valeur)`) qui dépose l'événement dans
   une file, et un thread Ocara qui appelle les gestionnaires enregistrés,
   l'un après l'autre, jamais sur le thread principal Android (§7 de
   l'analyse).
2. **Barres système** (couleur, masquage), **orientation**, **plein écran
   immersif**, **écran maintenu allumé**.
3. **Retour haptique** et **partage** vers une autre app
   (`Intent.ACTION_SEND`).
4. **Boîtes de dialogue** (alerte, confirmation) et **messages brefs**,
   avec les noms et les signatures prévus pour `ocara.UI` (règle de
   l'étape 1 : codés une fois dans `runtime_android/ui`, appelés ensuite par
   `ocara.UI`).
5. **Bouton retour** remonté à Ocara comme un événement (refusable).

Même schéma que les builtins `ocara.Android*` : crate
`runtime_android/ui` lié dans le `.so` seulement s'il est importé, classe
Kotlin `com.ocara.bridge.OcaraUI` (`packaging/android`), `AndroidException`
100 hors Android. Vérification sur téléphone.

## Plus tard (étape 4 du chantier)

Le moteur Compose qui rend un `.oc.ui` (`OcaraUI.kt`), partagé avec
`ocara.UI`, et les composants Material sans équivalent desktop (bouton
flottant, panneau inférieur, tiroir de navigation). Pistes écartées pour le
rendu natif : `ANativeActivity` (pas de composants natifs) et un pont JNI
widget par widget (surface énorme, propriété des objets Java depuis Ocara).

## Complexité

Structurel : le point 1 (appel du code Ocara depuis le thread principal
Android, par une file) est la partie délicate ; les points 2 à 5 sont des
appels Kotlin de quelques lignes chacun.

## Fichiers clés

[langage-builtin-ui-multiplateforme](langage-builtin-ui-multiplateforme.md)
(analyse, §7 et §8), `runtime_android_jni/` (pont JNI), `runtime_android/common/`
(appels Kotlin), `packaging/android/` (app Compose, `OcaraBridge.kt`),
[packaging-android-native-features](packaging-android-native-features.md)
(builtins Android hors interface, même schéma de crates).
