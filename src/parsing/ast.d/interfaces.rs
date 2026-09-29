/// Déclarations d'interfaces

use super::types::Type;
use super::params::Param;
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Méthode d'interface (signature seule)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceMethod {
    pub name:   String,
    pub params: Vec<Param>,
    pub ret_ty: Type,
    /// `true` si la signature exige une méthode STATIQUE chez
    /// l'implémentation (`public static method ...`) — nécessaire pour
    /// `wiring` : une interface ne déclarant que des contrats statiques doit
    /// pouvoir router `Interface::method()` vers la classe wired (voir
    /// docs/roadmap.d/langage-interface-wiring.md). Le modificateur `public`
    /// éventuel devant `method`/`static method` est accepté mais purement
    /// cosmétique (une méthode d'interface est par nature un contrat public,
    /// aucune notion de visibilité n'existe pour elle) — voir
    /// `parse_interface_method`.
    pub is_static: bool,
    pub span:   Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// `wiring <chemin.pointé.vers.Classe>` — liaison interface → implémentation
// (voir docs/roadmap.d/langage-interface-wiring.md)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct WiringDecl {
    /// Chemin qualifié complet tel qu'écrit (`context.home.infra.db.Foo` →
    /// `["context", "home", "infra", "db", "Foo"]`) — le NOM SIMPLE (dernier
    /// segment) est ce qui doit rester unique entre tous les `wiring` d'une
    /// même interface (voir la vérification à la déclaration de l'interface)
    /// et ce à quoi un alias d'import doit correspondre pour sélectionner ce
    /// wiring précis.
    pub path: Vec<String>,
    pub span: Span,
}

impl WiringDecl {
    /// Dernier segment du chemin qualifié — le nom réel de la classe visée,
    /// tel qu'il existe dans la table des symboles une fois le fichier
    /// chargé (`program.classes`/`SymbolTable::classes`, jamais préfixé par
    /// son chemin qualifié, comme toute classe utilisateur dans ce
    /// compilateur).
    pub fn simple_name(&self) -> &str {
        self.path.last().map(String::as_str).unwrap_or("")
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Déclaration d'interface
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceDecl {
    pub name:    String,
    pub methods: Vec<InterfaceMethod>,
    /// Zéro ou plusieurs `wiring <chemin>` — ordre TEXTUEL de déclaration
    /// préservé (premier élément = "premier wiring déclaré", significatif
    /// pour la résolution sans alias, voir langage-interface-wiring.md).
    pub wirings: Vec<WiringDecl>,
    pub span:    Span,
}
