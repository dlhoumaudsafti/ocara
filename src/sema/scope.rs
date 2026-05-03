use std::collections::HashMap;
use crate::parsing::ast::Type;
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Scope lexical empilé
// ─────────────────────────────────────────────────────────────────────────────

/// Un binding local (variable ou paramètre)
#[derive(Debug, Clone)]
pub struct LocalBinding {
    pub ty:      Type,
    pub mutable: bool,
    /// Déclaré à cette position (pour les warnings)
    pub span:    Span,
    /// Marqué vrai dès qu'on lit la variable
    pub used:    bool,
    /// Paramètre de fonction → pas de warning unused
    pub is_param: bool,
}

/// Variable non utilisée retournée par `pop_with_warnings`.
pub struct UnusedVar {
    pub name: String,
    pub span: Span,
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

    #[allow(dead_code)]
    pub fn pop(&mut self) {
        self.frames.pop();
    }

    /// Dépile le scope courant et retourne les variables non utilisées.
    pub fn pop_with_warnings(&mut self) -> Vec<UnusedVar> {
        let frame = match self.frames.pop() {
            Some(f) => f,
            None    => return vec![],
        };
        let mut unused: Vec<UnusedVar> = frame.into_iter()
            .filter(|(_, b)| !b.used && !b.is_param)
            .map(|(name, b)| UnusedVar { name, span: b.span.clone() })
            .collect();
        // Tri pour ordre déterministe (ligne, colonne)
        unused.sort_by_key(|u| (u.span.line, u.span.col));
        unused
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

    /// Marque une variable comme utilisée (en remontant la pile).
    pub fn mark_used(&mut self, name: &str) {
        for frame in self.frames.iter_mut().rev() {
            if let Some(b) = frame.get_mut(name) {
                b.used = true;
                return;
            }
        }
    }
}
