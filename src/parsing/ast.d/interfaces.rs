/// Déclarations d'interfaces

use super::types::Type;
use super::params::Param;
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Méthode d'interface (signature seule)
// ─────────────────────────────────────────────────────────────────────────────

/// Signature d'une méthode d'interface, avec ses modificateurs.
///
/// Grammaire symétrique à `ClassMember::Method` depuis
/// docs/roadmap.d/langage-interface-method-modifiers.md : `public`/
/// `private`/`protected` (au plus un, optionnel — contrairement à
/// `parse_visibility` pour une classe, où il est obligatoire) suivi de
/// `static`/`async` dans n'importe laquelle des 4 combinaisons (rien,
/// `static` seul, `async` seul, les deux ensemble) — 12 combinaisons au
/// total, toutes acceptées par `parse_interface_method`.
///
/// **Visibilité NON stockée ici, volontairement** : `FuncSig` (table des
/// symboles) ne porte AUCUN champ de visibilité pour quelque méthode que ce
/// soit dans ce compilateur (ni pour une classe ordinaire, ni pour
/// `extends`) — la visibilité n'a jamais fait partie d'une comparaison de
/// signature nulle part dans ce langage. Rendre la conformité `implements`
/// plus stricte que `extends` sur ce point précis serait une asymétrie
/// NOUVELLE, à l'opposé de l'objectif explicite de ce ticket (symétrie avec
/// `class`) ; sans corps de méthode par défaut dans une interface, il
/// n'existe de toute façon aucun cas d'usage concret motivant cette
/// sémantique aujourd'hui. Le token de visibilité est simplement consommé
/// par `parse_interface_method` puis jeté.
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceMethod {
    pub name:   String,
    pub params: Vec<Param>,
    pub ret_ty: Type,
    /// `true` si la signature exige une méthode STATIQUE chez
    /// l'implémentation (`public static method ...`) — nécessaire pour
    /// `wiring` : une interface ne déclarant que des contrats statiques doit
    /// pouvoir router `Interface::method()` vers la classe wired (voir
    /// docs/roadmap.d/langage-interface-wiring.md).
    pub is_static: bool,
    /// `true` si la méthode est déclarée `async` — sémantique directe
    /// (aucune ambiguïté, contrairement à la visibilité, voir la doc de
    /// `InterfaceMethod` ci-dessus) : identique à ce qu'une méthode de
    /// `class` fait déjà (`ClassMember::Method.decl.is_async`) — vérifié à
    /// la conformité `implements` comme `is_static`.
    pub is_async: bool,
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
