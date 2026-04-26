use std::collections::HashMap;
use crate::ast::Type;

// ─────────────────────────────────────────────────────────────────────────────
// Scope lexical empilé
// ─────────────────────────────────────────────────────────────────────────────

/// Un binding local (variable ou paramètre)
#[derive(Debug, Clone)]
pub struct LocalBinding {
    pub ty:      Type,
    pub mutable: bool,
}

/// Pile de scopes lexicaux.
/// Le sommet (index 0) est le scope le plus interne.
#[derive(Debug, Default)]
pub struct ScopeStack {
    frames: Vec<HashMap<String, LocalBinding>>,
}

impl ScopeStack {
    pub fn push(&mut self) {
        self.frames.push(HashMap::new());
    }

    pub fn pop(&mut self) {
        self.frames.pop();
    }

    /// Déclare un symbole dans le scope courant.
    /// Retourne `false` si le nom est déjà déclaré dans ce scope exact.
    pub fn declare(&mut self, name: String, binding: LocalBinding) -> bool {
        let top = self.frames.last_mut().expect("scope stack vide");
        if top.contains_key(&name) {
            return false;
        }
        top.insert(name, binding);
        true
    }

    /// Recherche en remontant la pile.
    pub fn lookup(&self, name: &str) -> Option<&LocalBinding> {
        for frame in self.frames.iter().rev() {
            if let Some(b) = frame.get(name) {
                return Some(b);
            }
        }
        None
    }
}
