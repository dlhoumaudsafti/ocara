/// Déclaration `architecture` du fichier d'entrée (voir
/// docs/roadmap.d/langage-mode-hexa.md et core::architecture).

use crate::parsing::token::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum ArchitectureMode {
    Hexagonal,
    Permissive,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArchitectureDecl {
    pub mode: ArchitectureMode,
    /// Répertoire structuré en hexagonal (`src: context`), chemin pointé.
    pub src: Option<(String, Span)>,
    /// (couche canonique, alias de répertoire, position).
    pub layers: Vec<(String, String, Span)>,
    /// Racines partagées, chemins pointés depuis la racine du projet.
    pub shared: Vec<(String, Span)>,
    pub span: Span,
}
