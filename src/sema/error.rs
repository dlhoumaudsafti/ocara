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
                format!("symbole indéfini '{}'", name),
            SemaError::TypeMismatch      { expected, found, .. } =>
                format!("type attendu '{}', trouvé '{}'", expected, found),
            SemaError::DuplicateSymbol   { name, .. } =>
                format!("symbole en double '{}'", name),
            SemaError::NotCallable       { name, .. } =>
                format!("'{}' n'est pas appelable", name),
            SemaError::WrongArgCount     { name, expected, found, .. } =>
                format!("'{}' attend {} argument(s), {} fourni(s)", name, expected, found),
            SemaError::ReturnTypeMismatch{ expected, found, .. } =>
                format!("retour attendu '{}', trouvé '{}'", expected, found),
            SemaError::NotAClass         { name, .. } =>
                format!("'{}' n'est pas une classe", name),
            SemaError::FieldNotFound     { class, field, .. } =>
                format!("champ '{}' introuvable dans la classe '{}'", field, class),
            SemaError::InterfaceNotImpl  { class, iface, method, .. } =>
                format!("classe '{}' n'implante pas '{}::{}' de l'interface '{}'", class, iface, method, iface),
            SemaError::InvalidAssign     { name, .. } =>
                format!("impossible d'assigner à '{}' (immuable ou non-déclaré)", name),
            SemaError::NotStaticMethod   { class, method, .. } =>
                format!("'{}::{}' n'est pas statique — utilisez une instance", class, method),
            SemaError::StaticOnInstance  { class, method, .. } =>
                format!("'{}' est statique — utilisez self::{}() depuis la classe ou {}::{}() depuis l'extérieur", method, method, class, method),
            SemaError::SelfOutsideClass  { .. } =>
                "erreur interne : self:: hors contexte de classe".into(),
            SemaError::MixedInProperty   { class, field, .. } =>
                format!("le type 'mixed' est interdit pour les champs de classe : '{}.{}' doit utiliser un type concret ou 'map<string, mixed>'", class, field),
            SemaError::MixedInReturnType { name, .. } =>
                format!("le type 'mixed' est interdit comme type de retour : '{}' doit retourner un type concret ou utiliser des unions (ex: int|string|null)", name),
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
                format!("variable '{}' jamais utilisée", name),
            SemaWarning::MixedLocalVariable { name, .. } =>
                format!("variable locale '{}' : le type 'mixed' désactive les vérifications de type — préférer un type concret ou une union (ex: int|string|null)", name),
            SemaWarning::VariadicMixed { name, .. } =>
                format!("paramètre variadic '{}' : variadic<mixed> désactive les vérifications de type — envisager variadic<T|U> avec union explicite", name),
        }
    }
}

impl fmt::Display for SemaWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.span(), self.message())
    }
}
