# Builtin `ocara.UIHybrid` (WebView unifiée : Tauri+WebKit desktop ↔ WebView Android)

## Idée

Généraliser [le hybride WebView Android](packaging-android-webview-hybrid.md) en un builtin Ocara à part entière, `ocara.UIHybrid` : une fenêtre hébergeant un moteur web, qu'il s'agisse de WebKit/GTK via Tauri sur desktop ou de la `WebView` système sur Android — même paradigme des deux côtés (une page, un pont JS↔Ocara), contrairement à [`ocara.UI`](langage-builtin-ui-multiplateforme.md) (GUI native, paradigmes structurellement différents entre desktop et Android).

C'est le point de départ le plus solide des deux builtins UI envisagés : Tauri (desktop) et la `WebView` Android sont déjà, chacun de leur côté, "une fenêtre + un moteur web + un pont natif" — l'unification est une question de plomberie (même API Ocara, deux implémentations qui se ressemblent déjà), pas de conception d'un plus petit dénominateur commun entre deux paradigmes incompatibles comme pour `ocara.UI`.

## Ce qui existe déjà de chaque côté

- **Desktop (Tauri)** : `ocara.Tauri` (`docs/builtins/Tauri.md`) — fenêtre WebView réelle, pont IPC JS→Ocara réel (`ui.handler`/`ui.handlers`), le reste (`listen`/`emit`/`dialog`/`notify`) simulé en mémoire (voir [builtins-tauri](builtins-tauri.md), "vrai support Tauri" pas encore fini).
- **Android** : rien encore — [le hybride WebView Android](packaging-android-webview-hybrid.md) (pont JNI + Activity + WebView) est lui-même non commencé, prérequis direct de ce ticket-ci.

## Ce qui resterait à concevoir

- Une API `ocara.UIHybrid` commune (créer la fenêtre, charger une URL/un contenu, enregistrer un pont JS↔Ocara, événements de fenêtre) qui se traduit soit vers `ocara.Tauri` (desktop), soit vers le pont JNI + WebView Android (mobile) selon `--target` — même mécanisme de résolution par cible que celui envisagé pour `ocara.UI`.
- Si un jour ce builtin existe, `ocara.Tauri` pourrait devenir un cas particulier/alias desktop de `ocara.UIHybrid` plutôt que deux API séparées — question à trancher le moment venu, pas maintenant.

## Priorité / Complexité

**Priorité Très Basse** — dépend de [packaging-android-webview-hybrid](packaging-android-webview-hybrid.md) (pas fait) et n'a de sens qu'une fois celui-ci en place. **Complexité Massive** — même mécanisme de résolution de builtin par cible que [langage-builtin-ui-multiplateforme](langage-builtin-ui-multiplateforme.md) (nouveau dans Ocara), plus la conception d'une API commune (plus simple ici qu'en GUI native, les deux paradigmes sous-jacents étant déjà proches).

## Fichiers clés

Aucun — chantier pas commencé. [packaging-android-webview-hybrid](packaging-android-webview-hybrid.md) (prérequis direct, pas fait), `docs/builtins/Tauri.md` (API desktop existante, référence), [langage-builtin-ui-multiplateforme](langage-builtin-ui-multiplateforme.md) (ticket frère pour la GUI native, même mécanisme de résolution par cible à concevoir une fois pour les deux).
