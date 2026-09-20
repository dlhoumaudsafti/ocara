# GUI native Android pilotée depuis Ocara

## Idée

Après [le hybride WebView](packaging-android-webview-hybrid.md) (étape "pour commencer"), aller plus loin : permettre à un programme Ocara de piloter une **interface graphique native Android** (vues Android/Jetpack Compose, ou rendu bas niveau via `ANativeWindow`/`ANativeActivity` du NDK) directement, pas seulement via une page HTML dans une `WebView`.

**Non tranché** — David a explicitement indiqué vouloir d'abord traiter le hybride WebView, "puis on verra" pour ce point. Ce ticket existe pour garder la trace de l'intention, pas pour fixer une direction technique : aucune des pistes ci-dessous n'a été évaluée en profondeur.

## Pistes possibles (aucune investiguée)

- **`ANativeActivity` / `ANativeWindow`** (NDK bas niveau, C) — dans l'esprit de ce que fait déjà `ocara.SDL` sur desktop : rendu direct dans une surface, sans passer par la JVM pour l'UI elle-même. Cohérent avec [packaging-android](packaging-android.md) sous-chantier 4 (SDL Android) si celui-ci aboutit un jour — SDL sait déjà s'intégrer à `ANativeActivity`, ce serait alors le MÊME mécanisme, pas un nouveau.
- **Pont JNI vers les Views Android / Jetpack Compose** (Java/Kotlin) — permettrait des composants natifs "comme il faut" (Material Design, accessibilité système, etc.), mais demande une bien plus grosse surface de pont JNI que le hybride WebView (chaque widget, chaque callback d'événement), et un nouveau builtin Ocara (`ocara.AndroidUI` ou similaire) entièrement à concevoir — API, ownership des objets Java depuis le modèle mémoire sans-GC d'Ocara (question ouverte, non triviale : voir la contrainte no-GC documentée pour le langage), etc.
- **Un builtin unique multiplateforme** (`ocara.UI` qui router vers Tauri sur desktop, GUI native sur Android) — idée séduisante mais spéculative, à ne considérer qu'une fois qu'au moins un backend Android concret existe.

## Priorité / Complexité

**Priorité Très Basse** — explicitement "on verra", pas de travail prévu avant que [le hybride WebView](packaging-android-webview-hybrid.md) soit fait. **Complexité non évaluée** — dépend entièrement de la piste choisie, qui n'a pas encore été discutée ni tranchée ; probablement Massive quelle que soit la piste (nouveau pont JNI large, ou nouveau backend SDL/NativeActivity, dans les deux cas une zone entièrement neuve pour ce projet).

## Fichiers clés

Aucun — chantier pas commencé, aucune piste tranchée. [packaging-android](packaging-android.md) et [packaging-android-webview-hybrid](packaging-android-webview-hybrid.md) (prérequis/contexte), `docs/builtins/Tauri.md`/`docs/builtins/SDL.md` (les deux builtins GUI existants, référence de style d'API pour un futur builtin Android).
