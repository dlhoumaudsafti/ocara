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
    SelfOutsideClass  { span: Span },
    MixedInProperty   { class: String, field: String, span: Span },
    MixedInReturnType { name: String, span: Span },
}

impl SemaError {
    pub fn span(&self) -> &Span {
        match self {
            SemaError::UndefinedSymbol    { span, .. } => span,
            SemaError::TypeMismatch       { span, .. } => span,
            SemaError::DuplicateSymbol    { span, .. } => span,
            SemaError::NotCallable        { span, .. } => span,
            SemaError::WrongArgCount      { span, .. } => span,
            SemaError::ReturnTypeMismatch { span, .. } => span,
            SemaError::NotAClass          { span, .. } => span,
            SemaError::FieldNotFound      { span, .. } => span,
            SemaError::InterfaceNotImpl   { span, .. } => span,
            SemaError::InvalidAssign      { span, .. } => span,
            SemaError::NotStaticMethod    { span, .. } => span,
            SemaError::StaticOnInstance   { span, .. } => span,
            SemaError::SelfOutsideClass   { span, .. } => span,
            SemaError::MixedInProperty    { span, .. } => span,
            SemaError::MixedInReturnType  { span, .. } => span,
        }
    }

    pub fn message(&self) -> String {
        match self {
            SemaError::UndefinedSymbol   { name, .. } =>
                format!("undefined symbol '{}'", name),
            SemaError::TypeMismatch      { expected, found, .. } =>
                format!("expected type '{}', found '{}'", expected, found),
            SemaError::DuplicateSymbol   { name, .. } =>
                format!("duplicate symbol '{}'", name),
            SemaError::NotCallable       { name, .. } =>
                format!("'{}' is not callable", name),
            SemaError::WrongArgCount     { name, expected, found, .. } =>
                format!("'{}' expects {} argument(s), {} provided", name, expected, found),
            SemaError::ReturnTypeMismatch{ expected, found, .. } =>
                format!("expected return type '{}', found '{}'", expected, found),
            SemaError::NotAClass         { name, .. } =>
                format!("'{}' is not a class", name),
            SemaError::FieldNotFound     { class, field, .. } =>
                format!("field '{}' not found in class '{}'", field, class),
            SemaError::InterfaceNotImpl  { class, iface, method, .. } =>
                format!("class '{}' does not implement '{}::{}' from interface '{}'", class, iface, method, iface),
            SemaError::InvalidAssign     { name, .. } =>
                format!("cannot assign to '{}' (immutable or undeclared)", name),
            SemaError::NotStaticMethod   { class, method, .. } =>
                format!("'{}::{}' is not static — use an instance", class, method),
            SemaError::StaticOnInstance  { class, method, .. } =>
                format!("'{}' is static — use self::{}() from within the class or {}::{}() from outside", method, method, class, method),
            SemaError::SelfOutsideClass  { .. } =>
                "internal error: self:: outside class context".into(),
            SemaError::MixedInProperty   { class, field, .. } =>
                format!("type 'mixed' is forbidden for class fields: '{}.{}' must use a concrete type or 'map<string, mixed>'", class, field),
            SemaError::MixedInReturnType { name, .. } =>
                format!("type 'mixed' is forbidden as return type: '{}' must return a concrete type or use unions (e.g., int|string|null)", name),
        }
    }
}

impl fmt::Display for SemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.span(), self.message())
    }
}

impl std::error::Error for SemaError {}

// ─────────────────────────────────────────────────────────────────────────────
// Avertissements sémantiques
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum SemaWarning {
    UnusedVariable { name: String, span: Span },
    MixedLocalVariable { name: String, span: Span },
    VariadicMixed { name: String, span: Span },
}

impl SemaWarning {
    pub fn span(&self) -> &Span {
        match self {
            SemaWarning::UnusedVariable { span, .. } => span,
            SemaWarning::MixedLocalVariable { span, .. } => span,
            SemaWarning::VariadicMixed { span, .. } => span,
        }
    }

    pub fn message(&self) -> String {
        match self {
            SemaWarning::UnusedVariable { name, .. } =>
                format!("variable '{}' is never used", name),
            SemaWarning::MixedLocalVariable { name, .. } =>
                format!("local variable '{}': type 'mixed' disables type checking — prefer a concrete type or union (e.g., int|string|null)", name),
            SemaWarning::VariadicMixed { name, .. } =>
                format!("variadic parameter '{}': variadic<mixed> disables type checking — consider variadic<T|U> with explicit union", name),
        }
    }
}

impl fmt::Display for SemaWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.span(), self.message())
    }
}
