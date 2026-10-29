// ─────────────────────────────────────────────────────────────────────────────
// Étiquettes des builtins bloquants
//
// - thread principal : boucle d'événements qui prend le thread principal
//   (`Tauri.run`, `UI.run`) ;
// - bloquant : bloque le thread appelant (serveur, attente, lecture…).
//
// Lues par `assertBlocksMainThread`/`assertIsBlocking` (src/sema/blocking_assert.rs).
// Dans un binaire de test, les builtins qui ne rendent jamais la main
// (`Tauri.run`, `HTTPServer.run`) appelés sur le thread principal rendent la
// main tout de suite (runtime/src/blocking.rs) ; les attentes bornées
// (`Thread::sleep`, `join`, lectures sur une entrée standard vide) s'exécutent.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Blocking {
    /// Bloque le thread appelant.
    Blocks,
    /// Occupe le thread principal (boucle d'événements) ; bloque aussi l'appelant.
    MainThread,
}

const MAIN_THREAD: &[(&str, &str)] = &[
    ("Tauri", "run"),
    ("UI", "run"),
];

const BLOCKS: &[(&str, &str)] = &[
    ("HTTPServer", "run"),
    ("Thread", "join"),
    ("Thread", "sleep"),
    ("Mutex", "lock"),
    ("Mutex", "withLock"),
    ("IO", "read"),
    ("IO", "readln"),
    ("IO", "readInt"),
    ("IO", "readFloat"),
    ("IO", "readBool"),
    ("IO", "readArray"),
    ("IO", "readMap"),
    ("HTTPRequest", "get"),
    ("HTTPRequest", "post"),
    ("HTTPRequest", "put"),
    ("HTTPRequest", "delete"),
    ("HTTPRequest", "patch"),
    ("HTTPRequest", "send"),
    ("SDL", "delay"),
    ("UI", "alert"),
    ("UI", "confirm"),
    ("UI", "pickFile"),
    ("UI", "saveFile"),
];

/// Étiquette de la méthode `method` du builtin `class`.
pub fn label(class: &str, method: &str) -> Option<Blocking> {
    let key = (class, method);
    if MAIN_THREAD.contains(&key) {
        Some(Blocking::MainThread)
    } else if BLOCKS.contains(&key) {
        Some(Blocking::Blocks)
    } else {
        None
    }
}
