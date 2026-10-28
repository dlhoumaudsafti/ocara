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

## À faire

3. **Événements réels** : `emit` (Ocara → page) et `listen` (page → Ocara),
   par le système d'événements de Tauri.
4. **Boîtes de dialogue natives** (plugin officiel `dialog` : message,
   confirmation, ouverture et enregistrement de fichier) et **notifications**
   (plugin officiel `notification`), à la place des simulations.
5. **Menus** : barre de menus et menu contextuel natifs ; contrôle du menu
   clic droit de WebKit et de l'inspecteur
   ([builtins-tauri-webkit-menu-inspecteur](builtins-tauri-webkit-menu-inspecteur.md)).
6. **Spécificités desktop** que garde `ocara.Tauri` : plusieurs fenêtres,
   options de fenêtre (sans bordure, toujours au premier plan, transparence),
   icône de zone de notification (tray), raccourcis globaux.

**Règle pour préparer `ocara.UI`** : ce qui fait partie du standard
(boîtes de dialogue, notifications, titre, taille et fermeture de fenêtre,
menus) est codé une seule fois dans `runtime_tauri`, avec les noms et les
signatures prévus pour `ocara.UI` (§3 et §6 de l'analyse) ; `ocara.Tauri`
l'expose dès maintenant, `ocara.UI` l'appellera ensuite sans réécriture.

Chaque point se termine par sa doc (`docs/builtins/Tauri.md`, section « Ce
qui est simulé » réduite d'autant), un exemple et un test.

## Complexité

Massive au total (six points), chacun Légère à Structurel. Le point 2
(agir sur la fenêtre depuis un autre thread que celui de `run()`) conditionne
les autres.

## Fichiers clés

`runtime_tauri/src/lib.rs`, `runtime_tauri/Cargo.toml` (plugins et features
Tauri), `src/builtins/tauri.rs` et `src/codegen/desc.d/tauri.rs`
(déclarations), `docs/builtins/Tauri.md`,
[builtins-tauri-webkit-menu-inspecteur](builtins-tauri-webkit-menu-inspecteur.md),
[langage-builtin-ui-multiplateforme](langage-builtin-ui-multiplateforme.md).
