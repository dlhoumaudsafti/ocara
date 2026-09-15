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
    /// `var`/`const` d'un type ressource (`Mutex`/`SQLite`/`MySQL`/`MariaDB`)
    /// prouvé "contenu" (jamais échappé — voir
    /// `crate::sema::escape::var_never_escapes`) qui atteint la fin de son
    /// bloc sans jamais avoir été finalisé manuellement (`.destroy()`/
    /// `.close()`) — `var`/`const` ne libèrent jamais rien automatiquement
    /// (contrairement à `scoped`/`consumed`), donc ce handle natif fuit pour
    /// toujours dès que la variable sort de portée.
    UnclosedResourceVar { name: String, ty_name: String, span: Span },
    /// `property` d'un type ressource (`Mutex`/`SQLite`/`MySQL`/`MariaDB`) —
    /// `__free_<Classe>` (voir `class_ownership::classify_field`) ne sait
    /// libérer que `Value`/une autre classe utilisateur, jamais une
    /// ressource : ce champ fuirait son handle natif à chaque libération de
    /// l'instance porteuse (`scoped`/`consumed`, ou un `var` auto-libéré).
    ResourceField { class: String, field: String, ty_name: String, span: Span },
    /// `string + T` avec `T` différent de `string` (et différent de `mixed`,
    /// qui échappe à cette vérification faute d'information statique, comme
    /// pour `comparable_types`/`orderable_types`). La concaténation `+` est
    /// strictement typée : seule `string + string` est autorisée. Toute
    /// conversion implicite passe par un template string ou `Convert::*ToStr`.
    StringConcatMismatch { left: String, right: String, span: Span },
    /// `use Foo<...>()` avec un nombre d'arguments de type incompatible avec
    /// les paramètres de type déclarés par `generic Foo<T, U=default>` —
    /// arité attendue : entre le nombre de paramètres sans valeur par défaut
    /// et le nombre total de paramètres déclarés.
    GenericArityMismatch { name: String, expected_min: usize, expected_max: usize, found: usize, span: Span },
    /// `.join()`/`.detach()` appelé une seconde fois sur la même `Thread` —
    /// use-after-free confirmé côté runtime (le premier appel a déjà repris
    /// et libéré le handle natif).
    ThreadAlreadyFinalized { name: String, span: Span },
    /// `on e is X` où `X` ne correspond à aucune classe connue (ni classe
    /// utilisateur du programme, ni classe d'exception builtin) — un typo
    /// rendait jusqu'ici ce handler silencieusement mort (jamais atteint,
    /// aucune erreur ni avertissement).
    OnFilterClassNotFound { name: String, span: Span },
    /// Handler catch-all (`on e { }`, sans `is`) pas en dernière position
    /// d'une chaîne `try`/`on` — il filtre déjà tout, les handlers suivants
    /// deviendraient morts (jamais atteints).
    CatchAllNotLast { span: Span },
    /// `.destroy()`/`.close()` appelé une seconde fois sur la même ressource
    /// (`Mutex`/`SQLite`/`MySQL`/`MariaDB`) — généralisation de
    /// `ThreadAlreadyFinalized` : SEGFAULT confirmé côté runtime sur un
    /// double `Mutex::destroy` (le premier appel a déjà libéré le handle
    /// natif).
    ResourceAlreadyFinalized { name: String, class_name: String, method: String, span: Span },
    /// `scoped`/`consumed` passée en argument à un constructeur/méthode
    /// UTILISATEUR connu dont ce paramètre est prouvé "retenu" au-delà de
    /// l'appel (stocké dans un champ, retourné, capturé par une closure...)
    /// — la source serait libérée en fin de bloc pendant que le callee en
    /// garde encore un alias (corruption mémoire silencieuse avant ce
    /// diagnostic, voir docs/roadmap.d/memoire-echappement-argument.md).
    ArgumentEscape { name: String, class_name: String, callee: String, span: Span },
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
            SemaError::UnclosedResourceVar { span, .. } => span,
            SemaError::ResourceField { span, .. } => span,
            SemaError::StringConcatMismatch { span, .. } => span,
            SemaError::GenericArityMismatch { span, .. } => span,
            SemaError::ThreadAlreadyFinalized { span, .. } => span,
            SemaError::OnFilterClassNotFound { span, .. } => span,
            SemaError::CatchAllNotLast { span } => span,
            SemaError::ResourceAlreadyFinalized { span, .. } => span,
            SemaError::ArgumentEscape      { span, .. } => span,
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
            SemaError::UnclosedResourceVar { name, ty_name, .. } =>
                format!("'{}' ('{}') is declared with 'var'/'const', never escapes its block, and is never '.destroy()'ed/'.close()'d — this native handle leaks permanently, since 'var'/'const' never close a resource automatically (unlike 'scoped'/'consumed'); call '.destroy()'/'.close()' explicitly, or declare it 'scoped'/'consumed' if you want the compiler to finalize it for you", name, ty_name),
            SemaError::ResourceField { class, field, ty_name, .. } =>
                format!("'{}.{}' ('{}') is a native resource field — it is never closed when a '{}' instance is destroyed (no mechanism exists for this today), so this handle always leaks; manage it outside the class instead, or expose an explicit method the caller must invoke before discarding the instance", class, field, ty_name, class),
            SemaError::StringConcatMismatch { left, right, .. } =>
                format!("cannot concatenate '{}' and '{}' with '+': string concatenation is strictly typed (only string + string is allowed) — use a template string (`${{...}}`) or convert explicitly (Convert::*ToStr)", left, right),
            SemaError::GenericArityMismatch { name, expected_min, expected_max, found, .. } =>
                if expected_min == expected_max {
                    format!("generic '{}' expects {} type argument(s), {} provided", name, expected_min, found)
                } else {
                    format!("generic '{}' expects between {} and {} type argument(s), {} provided", name, expected_min, expected_max, found)
                },
            SemaError::ThreadAlreadyFinalized { name, .. } =>
                format!("'{}' was already '.join()'ed or '.detach()'ed — calling either a second time would use a native handle already reclaimed", name),
            SemaError::OnFilterClassNotFound { name, .. } =>
                format!("'{}' is not a known class — this 'on e is {}' handler would never match anything", name, name),
            SemaError::CatchAllNotLast { .. } =>
                "a catch-all 'on' handler (without 'is') must be the last one in this try/on chain — handlers after it would never be reached".into(),
            SemaError::ResourceAlreadyFinalized { name, class_name, method, .. } =>
                format!("'{}' ('{}') was already '.{}()'ed — calling it a second time would use a native handle already reclaimed", name, class_name, method),
            SemaError::ArgumentEscape { name, class_name, callee, .. } =>
                format!("'{}' ('{}') is passed as an argument to '{}', which stores it beyond this call — a 'scoped'/'consumed' value cannot be passed where the callee retains it; clone it explicitly first, or pass a fresh value", name, class_name, callee),
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
