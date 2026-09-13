# Tauri : intégration explicitement partielle

`docs/builtins/Tauri.md` déclare lui-même le statut "partiellement fonctionnel". La fenêtre WebView et le pont IPC JS→Ocara sont réels ; en revanche `listen`, `emit`, `dialog`, `notify` et les getters/setters d'état de fenêtre après `run()` sont une **simulation en mémoire sans effet réel** (section "Ce qui est simulé" du document). C'est un stub assumé, pas un bug caché — mais un chantier important si l'objectif est un vrai support Tauri.

## Ampleur

Massif : nécessite de brancher réellement ces API sur les événements Tauri sous-jacents (`runtime_tauri/src/lib.rs`, 801 lignes).
