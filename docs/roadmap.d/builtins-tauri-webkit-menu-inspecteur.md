# Tauri : gestion du menu clic-droit et de l'inspecteur WebKit

## Constat

Aucun code de `runtime_tauri`/`ocara.Tauri` ne référence le menu contextuel (clic droit) ni l'inspecteur web (devtools) de la fenêtre WebView — vérifié par recherche dans `runtime_tauri/src/lib.rs` et `docs/builtins/Tauri.md`, aucune occurrence. Aujourd'hui, une fenêtre Tauri créée par un programme Ocara affiche donc le menu contextuel WebKit **par défaut du navigateur**, avec un accès non contrôlé à l'inspecteur (selon la configuration de build de `wry`/WebKitGTK) — aucun moyen, côté Ocara, de l'activer, le désactiver ou le personnaliser.

## Ce qui est vérifié dans `wry` (backend WebView de Tauri, version 0.55.1 résolue dans ce projet)

- **Inspecteur (devtools)** : `WebViewAttributes::devtools: bool` (champ générique, toutes plateformes) + `WebView::open_devtools()`/`close_devtools()` à l'exécution. Toujours actifs en debug ; en release, nécessite la feature Cargo `devtools` de la crate `tauri` — **non activée aujourd'hui** dans `runtime_tauri/Cargo.toml` (`features = ["wry", "compression"]`, pas `"devtools"`).
- **Menu contextuel** : `WebViewBuilderExtWindows::with_default_context_menus(bool)` existe mais est **`#[cfg(windows)]` uniquement** (spécifique à WebView2) — **aucune API haut niveau équivalente pour WebKitGTK** (le backend Linux réellement utilisé par ce projet, voir le shim pkg-config `webkit2gtk-4.1` dans le `Makefile`). Sur Linux, contrôler ou remplacer le menu contextuel demanderait de descendre au niveau de l'API C de WebKitGTK elle-même (signal GObject `context-menu` sur le `WebKitWebView`, ou `WebKitSettings`) — pas quelque chose que `wry` expose aujourd'hui de façon portable.

## Ce qui manque

- Décider de l'API Ocara souhaitée : un simple booléen (`Tauri::setDevToolsEnabled(bool)`/`Tauri::setContextMenuEnabled(bool)`), ou quelque chose de plus riche (menu personnalisé) — non tranché.
- Pour l'inspecteur : activer la feature Cargo `devtools` (changement simple, contenu dans `runtime_tauri/Cargo.toml`) puis exposer `open_devtools`/`close_devtools` comme méthodes du builtin.
- Pour le menu contextuel : sur Linux (la plateforme de développement actuelle de ce projet), nécessiterait une intégration FFI directe avec WebKitGTK (`webkit2gtk` crate ou bindings bruts) puisque `wry` n'expose rien de portable ici — plus gros que l'inspecteur, à évaluer séparément.

## Priorité / Complexité

**Priorité Très Basse** — confort de développement/production (masquer le menu clic-droit en release, contrôler l'accès à l'inspecteur), aucun besoin fonctionnel bloquant identifié aujourd'hui. **Complexité Légère** pour l'inspecteur (feature Cargo + deux méthodes) ; **complexité non évaluée, probablement Modérée à Massive** pour le menu contextuel sur Linux (FFI WebKitGTK direct, hors de l'API `wry` normalement utilisée par ce projet).

## Fichiers clés

`runtime_tauri/Cargo.toml` (feature `devtools` à activer), `runtime_tauri/src/lib.rs` (nouvelles méthodes du builtin), `docs/builtins/Tauri.md` (documentation, section "Ce qui est simulé"/statut), [builtins-tauri](builtins-tauri.md) (ticket parent "vrai support Tauri", même zone de code).
