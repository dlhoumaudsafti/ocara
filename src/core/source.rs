//! Lecture des fichiers source `.oc` pendant l'analyse. Le serveur de
//! langage y dépose le texte des documents ouverts dans l'éditeur, qui
//! remplace alors la version enregistrée sur disque.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

thread_local! {
    static OVERRIDES: RefCell<HashMap<PathBuf, String>> = RefCell::new(HashMap::new());
}

fn key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

pub fn read(path: &Path) -> std::io::Result<String> {
    if let Some(text) = OVERRIDES.with(|o| o.borrow().get(&key(path)).cloned()) {
        return Ok(text);
    }
    std::fs::read_to_string(path)
}

/// Remplace le contenu de `path` (texte non enregistré de l'éditeur).
pub fn set_override(path: &Path, text: String) {
    OVERRIDES.with(|o| o.borrow_mut().insert(key(path), text));
}

pub fn clear_override(path: &Path) {
    OVERRIDES.with(|o| o.borrow_mut().remove(&key(path)));
}
