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

## À faire

1. **État des lieux précis** de `runtime_tauri/src/lib.rs` : ce qui est
   réel, ce qui est simulé, et comment le code Ocara (thread principal pris
   par `run()`, gestionnaires IPC sur les threads de Tauri) peut agir sur la
   fenêtre (`AppHandle`, `run_on_main_thread`).
2. **État de fenêtre réel** : titre, taille, URL, fermeture, focus,
   réduction, agrandissement, appliqués à la vraie fenêtre, y compris depuis
   un gestionnaire pendant `run()`.
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
