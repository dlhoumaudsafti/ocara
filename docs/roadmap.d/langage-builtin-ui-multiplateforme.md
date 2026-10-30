# Builtin `ocara.UI` : suite

`ocara.UI` affiche un même fichier `.oc.ui` (YAML) en interface Tauri native
sur desktop et en Jetpack Compose sur Android. Ce qui existe est documenté
dans [docs/oc-ui.md](../oc-ui.md) (format, validation, composants, écrans
inclus, variables, tailles d'écran) et [docs/builtins/UI.md](../builtins/UI.md)
(API Ocara, vue web, composants Android). Les spécificités de chaque
plateforme restent dans `ocara.Tauri` et `ocara.AndroidUI`.

Ce fichier ne liste que ce qui reste à faire.

## 1. Menus du `.oc.ui`

`menu:` (racine) et `contextMenu:` (sur un nœud) sont validés et passent dans
l'arbre JSON (`src/core/ui_schema.d/nodes.rs`, `mod.rs`), mais **aucun
moteur ne les affiche** : ni `runtime_tauri/src/ui/engine.js`, ni le moteur
Compose (`packaging/android/app/src/main/java/com/ocara/bridge/OcaraUI*.kt`).

| | Desktop (Tauri) | Android (Compose) |
|---|---|---|
| `menu:` | barre de menus native, en réutilisant `runtime_tauri/src/menus.rs` d'`ocara.Tauri` | actions de la `TopAppBar` + menu de débordement |
| `contextMenu:` | menu natif au clic droit (même mécanisme que `ui.setContextMenu`) | `DropdownMenu` à l'appui long |
| `checked`, `enabled`, `shortcut`, `separator`, sous-menus | `CheckMenuItem`, raccourci natif | icône de coche ; raccourci ignoré |

Une entrée déclenche son `action`, liée par `ui.on` comme un bouton.
*(Légère)*

## 2. `webHandlers` du composant `webview`

`webHandlers` (nom → action) est validé (`src/core/ui_schema.d/schema.rs`)
mais pas câblé : `window.ocara.invoke(nom, args)` n'existe pas dans la page.

À faire : `invoke` rend une promesse résolue par la valeur de retour du
gestionnaire Ocara (ou rejetée par son exception). Desktop : IPC Tauri, même
mécanisme typé que `ui.handler` d'`ocara.Tauri` (`docs/builtins/Tauri.md`).
Android : `addJavascriptInterface` + réponse par `evaluateJavascript`. Le
contrat de retour d'un gestionnaire d'action (aujourd'hui `void`) est à
définir pour ce cas. *(Légère à Structurel)*

## 3. Vue web sous Windows et macOS

Hors Linux, `runtime_tauri/src/ui/mod.rs` prend `webview_stub.rs` : la place
du nœud `webview` reste vide. Poser une vue WebView2 (Windows) et WKWebView
(macOS) au rectangle du nœud, comme `webview.rs` le fait avec WebKitGTK.
Dépend de [packaging-windows.md](packaging-windows.md) (WebView2 exige MSVC)
et d'une machine macOS pour vérifier. *(Structurel)*

## 4. Fenêtre sans bordure contenant une vue web

Sur desktop, une fenêtre `decorations: false` qui contient une vue web ne se
redimensionne plus par ses bords (la vue posée par-dessus capte les
événements de la souris). Limite documentée dans `docs/builtins/UI.md`.
*(Légère)*

## 5. Questions de conception ouvertes

1. **Liaisons déclaratives** : l'état est aujourd'hui impératif
   (`ui.get`/`ui.set`, `bind:` dans les gabarits de liste seulement).
   Faut-il des liaisons sur un objet Ocara (`bind: user.name`), mises à jour
   dans les deux sens ?
2. **`ocara.Tauri` compilé pour Android** : aujourd'hui un talon sans effet
   (`src/codegen/emit.d/emitter.rs`, `is_tauri_stub`), pour qu'un appel sur
   un chemin mort compile. Lever une exception à l'appel, comme
   `ocara.Android*` hors Android (`AndroidException` 100), pour que les trois
   builtins se comportent pareil ?
