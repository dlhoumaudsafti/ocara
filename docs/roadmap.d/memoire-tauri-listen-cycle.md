# Fuite : callback `listen` qui capture sa fenêtre `Tauri`, sans `run()`

Statut : **à faire** (constaté le 2026-10-28).

## Constat

```ocara
import ocara.Tauri

function main(): void {
    const ui:Tauri = use Tauri({ "title": "x", "url": "index.html" })
    ui.listen("a", nameless(data:string): void {
        ui.setTitle(data)
    })
}
```

`--leak-check` : la fenêtre, la closure, son environnement et la cellule de
`ui` ne sont jamais libérés. La fenêtre retient la closure (état natif,
`runtime_tauri/src/events.rs`), la closure retient `ui` : un cycle que le
collecteur ne voit pas, l'état natif n'étant pas un champ de l'objet.

Avec `run()`, aucune fuite : `run()` rend les callbacks à la fermeture
(`finalize_window`), ce qui casse le cycle (vérifié avec
`examples/advanced/tauri_events/`). La fuite ne concerne donc qu'un programme
qui enregistre un écouteur capturant la fenêtre sans jamais appeler `run()`.

## Pistes

- Rendre les callbacks quand le dernier lien Ocara vers la fenêtre disparaît
  (compte de références de l'objet sans les captures de ses propres
  callbacks), ou faire connaître au collecteur de cycles les callbacks
  retenus (champ compté de l'objet `Tauri` plutôt qu'état natif).

## Fichiers clés

`runtime_tauri/src/events.rs`, `runtime_tauri/src/lib.rs` (`finalize_window`),
`runtime/src/rc.rs` (`finalize_object`, lettre `W`).
