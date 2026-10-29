# Fuite : callback `listen` qui capture sa fenêtre `Tauri`, sans `run()`

Statut : **corrigé** le 2026-10-29 (constaté le 2026-10-28).

## Correctif

Les objets `Tauri` et `UI` ont un second mot compté, `__kept` (masque
`W1`, `src/lower/builder.d/rc_layout.rs`) : un tableau Ocara des callbacks
que l'état natif de la fenêtre appelle. `ui.listen` (`runtime_tauri/src/events.rs`)
et `ui.on` (`runtime_ui/src/lib.rs`) y rangent le callback par
`__rc_keep(objet, 1, callback)` (`runtime/src/rc.rs`) ; l'état natif n'en
garde qu'une référence empruntée et le finaliseur ne les rend plus. Le
tableau est un enfant ordinaire de l'objet : le collecteur voit le cycle
fenêtre → callback → fenêtre capturée et le libère, et sans cycle les
callbacks partent avec l'objet. Un crochet du collecteur sur l'état natif a
été écarté : il aurait verrouillé l'état d'une fenêtre pendant la collecte.

Test : `examples/75_window_callback_cycle.oc` (`listen` et `on` capturant
leur fenêtre, sans `run()`), vérifié par `./ci/leak_check.sh`.

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
