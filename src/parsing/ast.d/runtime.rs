/// Blocs runtime et imports runtime

use super::statements::Stmt;
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Type de bloc runtime
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum RuntimeBlockKind {
    Init,
    Main,
    Error,
    Success,
    Exit,
}

#[allow(dead_code)]
impl RuntimeBlockKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuntimeBlockKind::Init => "init",
            RuntimeBlockKind::Main => "main",
            RuntimeBlockKind::Error => "error",
            RuntimeBlockKind::Success => "success",
            RuntimeBlockKind::Exit => "exit",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Import d'un fichier runtime
// ─────────────────────────────────────────────────────────────────────────────

/// Import d'un fichier runtime : `runtime start.Init as init`
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct RuntimeImport {
    /// Chemin du runtime : ["logger"] pour `runtime logger`
    pub path: Vec<String>,
    /// Type de bloc runtime cible (optionnel)
    /// - None : importer tous les blocs du fichier (ex: `runtime logger`)
    /// - Some(kind) : le contenu du fichier devient le bloc spécifié (ex: `runtime logger is init`)
    pub kind: Option<RuntimeBlockKind>,
    pub span: Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Bloc runtime
// ─────────────────────────────────────────────────────────────────────────────

/// Bloc runtime (init, main, error, success, exit)
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct RuntimeBlock {
    /// Type de bloc
    pub kind: RuntimeBlockKind,
    /// Statements du bloc (après expansion des imports runtime)
    pub statements: Vec<Stmt>,
    pub span: Span,
}
