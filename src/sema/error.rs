use std::fmt;
use crate::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Erreurs sémantiques
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SemaError {
    UndefinedSymbol   { name: String, span: Span },
    TypeMismatch      { expected: String, found: String, span: Span },
    DuplicateSymbol   { name: String, span: Span },
    NotCallable       { name: String, span: Span },
    WrongArgCount     { name: String, expected: usize, found: usize, span: Span },
    ReturnTypeMismatch{ expected: String, found: String, span: Span },
    NotAClass         { name: String, span: Span },
    FieldNotFound     { class: String, field: String, span: Span },
    InterfaceNotImpl  { class: String, iface: String, method: String, span: Span },
    InvalidAssign     { name: String, span: Span },
    NotStaticMethod   { class: String, method: String, span: Span },
    StaticOnInstance  { class: String, method: String, span: Span },
}

impl fmt::Display for SemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SemaError::UndefinedSymbol   { name, span } =>
                write!(f, "[{}] symbole indéfini '{}'", span, name),
            SemaError::TypeMismatch      { expected, found, span } =>
                write!(f, "[{}] type attendu '{}', trouvé '{}'", span, expected, found),
            SemaError::DuplicateSymbol   { name, span } =>
                write!(f, "[{}] symbole en double '{}'", span, name),
            SemaError::NotCallable       { name, span } =>
                write!(f, "[{}] '{}' n'est pas appelable", span, name),
            SemaError::WrongArgCount     { name, expected, found, span } =>
                write!(f, "[{}] '{}' attend {} argument(s), {} fourni(s)", span, name, expected, found),
            SemaError::ReturnTypeMismatch{ expected, found, span } =>
                write!(f, "[{}] retour attendu '{}', trouvé '{}'", span, expected, found),
            SemaError::NotAClass         { name, span } =>
                write!(f, "[{}] '{}' n'est pas une classe", span, name),
            SemaError::FieldNotFound     { class, field, span } =>
                write!(f, "[{}] champ '{}' introuvable dans la classe '{}'", span, field, class),
            SemaError::InterfaceNotImpl  { class, iface, method, span } =>
                write!(f, "[{}] classe '{}' n'implante pas '{}::{}' de l'interface '{}'",
                    span, class, iface, method, iface),
            SemaError::InvalidAssign     { name, span } =>
                write!(f, "[{}] impossible d'assigner à '{}' (immuable ou non-déclaré)", span, name),            SemaError::NotStaticMethod   { class, method, span } =>
                write!(f, "[{}] '{}::{}' n'est pas statique — utilisez une instance", span, class, method),
            SemaError::StaticOnInstance  { class, method, span } =>
                write!(f, "[{}] '{}::{}' est statique — appelez-la via {}::{} sans instance", span, class, method, class, method),        }
    }
}

impl std::error::Error for SemaError {}
