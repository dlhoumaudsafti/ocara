# Interface native, étape 1 : finaliser `ocara.Tauri`

Statut : **à faire, première étape** du chantier interface native
([`ocara.UI`](langage-builtin-ui-multiplateforme.md), §10). Indépendante de
l'étape 2 ([`ocara.AndroidUI`](packaging-android-gui-native.md)), utile seule
pour les programmes qui utilisent déjà `ocara.Tauri`.

## Constat

`docs/builtins/Tauri.md` déclare le builtin « partiellement fonctionnel ».
La fenêtre WebView et les appels JS → Ocara (`ui.handler`/`ui.handlers`)
sont réels. En revanche `listen`, `emit`, `dialog`, `notify` et les
lectures/modifications de l'état de fenêtre après `run()` (titre, taille,
URL, fermeture, focus, réduction, agrandissement) sont une **simulation en
mémoire sans effet** sur la vraie fenêtre (`runtime_tauri/src/lib.rs`).

## Fait (2026-10-28)

1. **État des lieux** : seuls la fenêtre (`Tauri_run`) et l'IPC JS → Ocara
   étaient réels. Les handlers IPC s'exécutent sur le thread principal, dans
   la boucle de Tauri ; Tauri y traite directement un appel de fenêtre (pas
   d'interblocage), et depuis un autre thread il le transmet à sa boucle.
2. **État de fenêtre réel** (`runtime_tauri/src/window.rs`) : titre, taille,
   URL, ouverture, fermeture, focus, réduction, agrandissement, restauration
   agissent sur la vraie fenêtre pendant `run()`, depuis un handler ou un
   autre thread ; avant `run()`, ils règlent la fenêtre à ouvrir. Vérifié
   sur une vraie fenêtre (GNOME, X11) par `examples/advanced/tauri_window/`.
   Corrigé au passage :
   - `run()` terminait le processus à la fermeture (`Builder::run`) : le code
     après `ui.run()` ne s'exécutait jamais ; `run_return` le rend désormais ;
   - `ui.minimize()` était compilé en `JSON_minimize(ui)` : le sucre JSON
     (`.encode()`, `.decode()`, `.pretty()`, `.minimize()`) s'appliquait à
     tout récepteur, même une classe qui déclare la méthode
     (`src/lower/expr.d/lower.rs`) ;
   - GNOME ignore la désiconification et met en « demande l'attention » une
     fenêtre présentée sans heure d'interaction : `open()`/`restore()`
     appellent `present_with_time` de GTK avec l'heure du serveur X.

3. **Événements réels** (`runtime_tauri/src/events.rs`) : `ui.emit(event,
   data)` encode `data` en JSON et le remet aux écouteurs
   `window.ocara.on(event, cb)` de la page (`eval` sur la vraie fenêtre) ;
   `window.ocara.emit(event, data)` passe par la commande IPC réservée
   `__ocara_event` et appelle chaque callback `ui.listen` avec le texte JSON.
   Une exception d'un callback est rattrapée et signalée, la boucle continue
   (`__call_closure_catching`, en ABI C : `runtime_tauri` embarque sa propre
   copie du runtime, dont la pile des `try` n'est pas celle de
   `__ocara_fail`). Vérifié par `examples/advanced/tauri_events/`.
   Corrigé au passage : l'objet `Tauri` n'était jamais libéré (pas compté) ;
   il est désormais compté et finalisé (lettre `W`, finaliseur enregistré
   par `runtime_tauri`). Reste une fuite si un écouteur capture la fenêtre et
   que `run()` n'est jamais appelé ([memoire-tauri-listen-cycle](memoire-tauri-listen-cycle.md)).

4. **Boîtes de dialogue et notifications natives**
   (`runtime_tauri/src/dialogs.rs`, plugins officiels `tauri-plugin-dialog`
   2.7 et `tauri-plugin-notification` 2.4, les dernières compatibles avec
   Tauri 2.11 ; Tauri épinglé en `~2.11`). Noms standard d'`ocara.UI` :
   `alert`, `confirm`, `pickFile`, `saveFile`, `notify(title, text)` (la forme
   `notify(options)`, simulée, est remplacée) ; `dialog(options)` conservée,
   désormais réelle. Depuis le thread de la boucle (handler, écouteur), la
   réponse est attendue en faisant tourner la boucle GTK (une attente simple
   bloquerait la boîte). Hors de `run()` : `TauriException` 102 ; levée en ABI
   C (`__raise_builtin_exception`) pour la même raison que les écouteurs.
   Vérifié sur une vraie fenêtre (confirm depuis un handler, alert, pickFile
   et saveFile depuis un thread, notification GNOME).

5. **Menus natifs** (`runtime_tauri/src/menus.rs`) : `setMenu(items)` (barre
   de menus, raccourcis, entrées cochables, sous-menus) et
   `setContextMenu(items)` (remplace le menu clic droit de WebKit, `[]` le
   supprime ; le clic droit est intercepté par le script injecté, ce qui
   marche aussi sous WebKitGTK, où `wry` n'a pas d'option portable). Un clic
   appelle les callbacks `ui.listen(action, …)` — même notion d'« action »
   qu'`ocara.UI`. Inspecteur web : `setDevTools(true)` / `openDevTools()`
   (feature `devtools` de Tauri ; désactivé par défaut). Vérifié sur une vraie
   fenêtre (barre, entrée cochable, menu contextuel, inspecteur).

## À faire

6. **Spécificités desktop** que garde `ocara.Tauri` : plusieurs fenêtres,
   options de fenêtre (sans bordure, toujours au premier plan, transparence),
   icône de zone de notification (tray), raccourcis globaux.

**Règle pour préparer `ocara.UI`** : ce qui fait partie du standard
(boîtes de dialogue, notifications, titre, taille et fermeture de fenêtre,
menus) est codé une seule fois dans `runtime_tauri`, avec les noms et les
signatures prévus pour `ocara.UI` (§3 et §6 de l'analyse) ; `ocara.Tauri`
l'expose dès maintenant, `ocara.UI` l'appellera ensuite sans réécriture.

Chaque point se termine par sa doc (`docs/builtins/Tauri.md`), un exemple
et sa vérification sur une vraie fenêtre (ocaraunit n'ouvre pas de fenêtre).

## Complexité

Massive au total (six points), chacun Légère à Structurel. Le point 2
(agir sur la fenêtre depuis un autre thread que celui de `run()`) conditionne
les autres.

## Fichiers clés

`runtime_tauri/src/lib.rs`, `runtime_tauri/Cargo.toml` (plugins et features
Tauri), `src/builtins/tauri.rs` et `src/codegen/desc.d/tauri.rs`
(déclarations), `docs/builtins/Tauri.md`,
[langage-builtin-ui-multiplateforme](langage-builtin-ui-multiplateforme.md).
