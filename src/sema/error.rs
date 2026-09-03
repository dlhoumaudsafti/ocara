use std::fmt;
use crate::parsing::token::Span;

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
    ResultOutsideRuntimeBlock { span: Span },
    ReturnInsideRuntimeBlock  { span: Span },
    IncomparableTypes { op: String, left: String, right: String, span: Span },
    /// `consumed x` lue/utilisée une seconde fois — elle est détruite après
    /// sa première utilisation, toute réutilisation ultérieure est invalide.
    ConsumedUsedTwice { name: String, first_use: Span, span: Span },
    /// `scoped`/`consumed` sur une ressource (Mutex/SQLite/MySQL/Thread) qui
    /// s'échappe de son bloc (affectation, `return`, argument autre que
    /// `self`) — un handle de ressource ne peut pas être cloné ni partagé.
    ResourceEscape { name: String, class_name: String, span: Span },
    /// `scoped`/`consumed Thread` en fin de bloc sans `.join()`/`.detach()`
    /// préalable — le compilateur ne peut pas choisir à la place du
    /// développeur entre attendre le thread et le détacher.
    ThreadNotFinalized { name: String, span: Span },
    /// `scoped`/`consumed` sur un type non pris en charge par ce chantier
    /// (primitif, SDL/Tauri, instance de classe utilisateur).
    OwnershipNotSupported { name: String, ty_name: String, span: Span },
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
            SemaError::ResultOutsideRuntimeBlock { span } => span,
            SemaError::ReturnInsideRuntimeBlock  { span } => span,
            SemaError::IncomparableTypes  { span, .. } => span,
            SemaError::ConsumedUsedTwice  { span, .. } => span,
            SemaError::ResourceEscape     { span, .. } => span,
            SemaError::ThreadNotFinalized { span, .. } => span,
            SemaError::OwnershipNotSupported { span, .. } => span,
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
            SemaError::ResultOutsideRuntimeBlock { .. } =>
                "'result' can only be used inside a runtime block (init/main/error/success/exit)".into(),
            SemaError::ReturnInsideRuntimeBlock { .. } =>
                "'return' is not allowed inside a runtime block — use 'result' instead to set ERROR without exiting".into(),
            SemaError::IncomparableTypes { op, left, right, .. } =>
                format!("cannot compare '{}' and '{}' with '{}': comparisons are strictly typed (int and float are the only compatible pair) — convert one side explicitly", left, right, op),
            SemaError::ConsumedUsedTwice { name, first_use, .. } =>
                format!("'{}' is 'consumed' and was already used at {} — it was destroyed right after that first use", name, first_use),
            SemaError::ResourceEscape { name, class_name, .. } =>
                format!("'{}' ('{}') cannot escape its 'scoped'/'consumed' block (assignment, return, or argument) — resource handles cannot be cloned or shared, use it locally via its own methods", name, class_name),
            SemaError::ThreadNotFinalized { name, .. } =>
                format!("'{}' is a 'scoped'/'consumed' Thread that reaches the end of its block without a call to '.join()' or '.detach()' — pick one explicitly", name),
            SemaError::OwnershipNotSupported { name, ty_name, .. } =>
                format!("'scoped'/'consumed' is not supported on '{}' for '{}' yet", ty_name, name),
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
