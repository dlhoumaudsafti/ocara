/// Déclarations d'imports et constantes globales

use super::types::Type;
use super::expressions::Expr;
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Déclaration de constante globale
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ConstDecl {
    pub name:  String,
    pub ty:    Type,
    pub value: Expr,
    pub span:  Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Import
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    /// Chemin qualifié ou noms à importer
    /// - Format ancien: `["ocara", "IO"]` pour `import ocara.IO`
    /// - Format nouveau: `["Circle"]` pour `import Circle from "file"`
    /// - Format wildcard: `["*"]` pour `import * from "file"`
    pub path:  Vec<String>,
    /// Chemin de fichier optionnel pour `from "path"`
    /// Ex: Some("11_interfaces.oc") pour `import Circle from "11_interfaces.oc"`
    pub file_path: Option<String>,
    /// Alias optionnel : `as UserData`
    pub alias: Option<String>,
    pub span:  Span,
}
