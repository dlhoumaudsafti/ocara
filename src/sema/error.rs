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
    /// `.close()`/`.destroy()`/`.closeResponse()` appelé manuellement sur
    /// `self.<champ>`, un champ de type ressource (`Mutex`/`SQLite`/`MySQL`/
    /// `MariaDB`/`HTTPRequest`/`HTTPResponse`) — `__free_<Classe>` ferme déjà
    /// ce champ automatiquement quand l'instance porteuse est détruite (voir
    /// `class_ownership::classify_field`/`FieldOwnership::Resource`) ; un
    /// appel manuel en plus referait une fermeture déjà faite ailleurs dans
    /// la vie de l'instance, SEGFAULT le jour où les deux se produisent
    /// (même famille que `ResourceAlreadyFinalized`, mais entre méthodes
    /// plutôt qu'au sein d'un seul bloc — pas de suivi possible à travers
    /// des appels de méthode arbitraires, donc rejeté d'office).
    ManualCloseOnResourceField { class: String, field: String, ty_name: String, method: String, span: Span },
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
    /// `var`/`scoped`/`consumed x:message<T>` — `message<T>` (générateurs,
    /// voir docs/roadmap.d/langage-emit-iterable.md) n'est JAMAIS nommable :
    /// il n'a de sens que comme type de retour déclaré d'une fonction/méthode
    /// contenant `emit`, jamais comme type d'un binding.
    MessageNotNameable { name: String, span: Span },
    /// `message<T>` utilisé comme type d'un paramètre — return-type-only.
    MessageAsParamType { name: String, span: Span },
    /// Fonction/méthode déclarant `message<T>` en retour mais dont le corps
    /// ne contient aucun `emit` atteignable — `message<T>` n'a de sens que
    /// pour une fonction qui émet réellement.
    MessageReturnWithoutEmit { name: String, span: Span },
    /// `emit` utilisé dans une fonction/méthode dont le type de retour
    /// déclaré n'est pas `message<T>`.
    EmitOutsideMessageFunction { span: Span },
    /// Consommation scalaire directe (`var x:T = f()`, `IO::writeln(f())`...)
    /// d'un `message<T>` dont au moins un `emit` est atteignable À
    /// L'INTÉRIEUR d'une boucle — le compilateur ne peut plus prouver
    /// statiquement qu'au plus une valeur est jamais produite (voir
    /// `crate::sema::message_emit`) : seuls `for`/`Array::fromMessage`
    /// restent valables dans ce cas.
    MessageUnsafeScalarConsumption { name: String, span: Span },
    /// `i++`/`++i`/`i--`/`--i` (`Expr::IncDec`) sur une cible dont le type
    /// n'est ni `int` ni `float` — voir docs/roadmap.d/langage-increment-decrement.md.
    IncDecInvalidType { found: String, span: Span },
    /// `expr.méthode(...)` où `expr` est de type `void` — typiquement
    /// `self.port(8080).workers(4)`, `port()` ne retournant rien à chaîner.
    /// Avant ce diagnostic, `type_class_name(Type::Void)` retombait sur
    /// `None` comme pour n'importe quel type non reconnu, silencieusement
    /// permissif (retourne `Type::Mixed` sans vérifier `field`/`args`) — le
    /// reste de la chaîne (ici `.workers(4)`) ne mangle alors vers AUCUN
    /// symbole existant et est ignoré par le codegen, sans la moindre
    /// erreur de compilation. Voir
    /// docs/roadmap.d/langage-appel-methode-sur-void-accepte.md.
    MethodCallOnVoid { method: String, span: Span },
    /// `expr.méthode(...)` où `expr` est de type `int`/`float`/`bool`/`null`/
    /// `message<T>`/`Function<...>` — aucun de ces types n'a de classe
    /// associée, donc aucune méthode. Même mécanisme que `MethodCallOnVoid`
    /// (E36), généralisé aux types que ce correctif avait délibérément
    /// laissés de côté : `type_class_name` retombait sur `None` pour eux
    /// aussi, silencieusement permissif (retourne `Type::Mixed` sans
    /// vérifier `field`/`args`). `Type::Mixed` n'est PAS concerné : son
    /// imprécision est un choix de langage assumé (désactive volontairement
    /// la vérification de types), pas un oubli. Voir
    /// docs/roadmap.d/langage-appel-methode-sur-primitif-accepte.md.
    MethodCallOnNonClass { type_name: String, method: String, available: Vec<String>, span: Span },
    /// `use Interface(...)` (construction) ou `Interface::method()` (appel
    /// statique) sur une interface qui n'a AUCUN `wiring` déclaré — voir
    /// docs/roadmap.d/langage-interface-wiring.md. Quand l'interface a au
    /// moins un `wiring`, `core::interface_wiring::resolve_bare_interface_names`
    /// réécrit déjà `class` vers la première classe concrète wired AVANT le
    /// typecheck (voir §4b-bis dans `main.rs`) : si ce nœud désigne encore
    /// une interface ICI, c'est nécessairement qu'aucun `wiring` n'existe —
    /// diagnostic dédié, plus parlant que `NotAClass`/un échec silencieux
    /// (`StaticCall` retombait jusqu'ici sur `Type::Mixed` sans la moindre
    /// erreur, une interface n'étant jamais cherchée dans `self.classes`).
    InterfaceNoWiring { name: String, span: Span },
    /// `resolve expr` où `expr` n'est PAS de type `Resolvable<T>` — voir
    /// docs/roadmap.d/langage-async-non-int-return-type-check.md.
    /// Auparavant, `Expr::Resolve` retombait silencieusement sur `Type::Int`
    /// dès que le mécanisme `async_var_funcs` (table par nom de variable,
    /// aujourd'hui supprimée) ne retrouvait pas l'appel async d'origine ;
    /// `Resolvable<T>` porte maintenant l'information dans le type lui-même,
    /// donc `resolve` sur autre chose qu'un `Resolvable<T>` est une vraie
    /// erreur de type, plus un repli silencieux.
    ResolveOnNonResolvable { found: String, span: Span },
    /// Type de retour DÉCLARÉ d'une fonction/méthode `async` qui est
    /// lui-même `Resolvable<T>` — interdit pour empêcher le double emballage
    /// implicite `Resolvable<Resolvable<T>>` (voir
    /// docs/roadmap.d/langage-async-non-int-return-type-check.md).
    AsyncReturnsResolvable { name: String, span: Span },
    /// Arguments nommés — voir `crate::sema::named_args` (E45 à E50).
    NamedArgMixed      { callee: String, span: Span },
    NamedArgUnknown    { callee: String, name: String, valid: Vec<String>, span: Span },
    NamedArgDuplicate  { callee: String, name: String, span: Span },
    NamedArgVariadic   { callee: String, name: String, span: Span },
    NamedArgMissing    { callee: String, name: String, span: Span },
    NamedArgUnresolved { name: String, span: Span },
    /// Champ `private`/`protected` lu ou affecté hors de la classe déclarante
    /// (et, pour `protected`, de ses descendantes) — voir
    /// `crate::sema::field_visibility` (E54).
    FieldNotAccessible { class: String, field: String, protected: bool, span: Span },
    /// Valeur d'une constante de classe non évaluable à la compilation
    /// (appel, variable...) — voir `Expr::const_literal` (E55).
    ClassConstNotConstant { class: String, name: String, span: Span },
    /// Valeur d'une constante globale non évaluable à la compilation (E61) :
    /// elle est réévaluée à l'entrée de chaque fonction, un appel y bouclerait.
    GlobalConstNotConstant { name: String, span: Span },
    /// Champ appelé comme une méthode (`e.message()` au lieu de
    /// `e.message`) — E57.
    FieldCalledAsMethod { class: String, field: String, span: Span },
    /// Opérateur arithmétique sur un opérande non numérique (`bool`, `array`,
    /// `map`, et `string` hors `+` entre chaînes et `string -= string`) — E59.
    ArithmeticOnNonNumeric { op: String, operand: String, span: Span },
    /// `use C(args)` sur une classe utilisateur sans `init` (ni hérité d'un
    /// ancêtre utilisateur) — les arguments seraient perdus (E60).
    ArgsWithoutConstructor { class: String, found: usize, span: Span },
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
            SemaError::ManualCloseOnResourceField { span, .. } => span,
            SemaError::StringConcatMismatch { span, .. } => span,
            SemaError::GenericArityMismatch { span, .. } => span,
            SemaError::ThreadAlreadyFinalized { span, .. } => span,
            SemaError::OnFilterClassNotFound { span, .. } => span,
            SemaError::CatchAllNotLast { span } => span,
            SemaError::ResourceAlreadyFinalized { span, .. } => span,
            SemaError::MessageNotNameable  { span, .. } => span,
            SemaError::MessageAsParamType  { span, .. } => span,
            SemaError::MessageReturnWithoutEmit { span, .. } => span,
            SemaError::EmitOutsideMessageFunction { span } => span,
            SemaError::MessageUnsafeScalarConsumption { span, .. } => span,
            SemaError::IncDecInvalidType { span, .. } => span,
            SemaError::MethodCallOnVoid { span, .. } => span,
            SemaError::MethodCallOnNonClass { span, .. } => span,
            SemaError::InterfaceNoWiring   { span, .. } => span,
            SemaError::ResolveOnNonResolvable { span, .. } => span,
            SemaError::AsyncReturnsResolvable { span, .. } => span,
            SemaError::NamedArgMixed      { span, .. } => span,
            SemaError::NamedArgUnknown    { span, .. } => span,
            SemaError::NamedArgDuplicate  { span, .. } => span,
            SemaError::NamedArgVariadic   { span, .. } => span,
            SemaError::NamedArgMissing    { span, .. } => span,
            SemaError::NamedArgUnresolved { span, .. } => span,
            SemaError::FieldNotAccessible { span, .. } => span,
            SemaError::ClassConstNotConstant { span, .. } => span,
            SemaError::GlobalConstNotConstant { span, .. } => span,
            SemaError::FieldCalledAsMethod { span, .. } => span,
            SemaError::ArithmeticOnNonNumeric { span, .. } => span,
            SemaError::ArgsWithoutConstructor { span, .. } => span,
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
                format!("'{}' ('{}') is declared with 'var'/'const', never escapes its block, and is never '.destroy()'/'.close()' — this native handle leaks permanently, since 'var'/'const' never close a resource automatically (unlike 'scoped'/'consumed'); call '.destroy()'/'.close()' explicitly, or declare it 'scoped'/'consumed' if you want the compiler to finalize it for you", name, ty_name),
            SemaError::ManualCloseOnResourceField { class, field, ty_name, method, .. } =>
                format!("'self.{}' ('{}') is closed automatically when the '{}' instance is destroyed — calling '.{}()' manually here would close it a second time (undefined behavior); remove this call", field, ty_name, class, method),
            SemaError::StringConcatMismatch { left, right, .. } =>
                format!("cannot concatenate '{}' and '{}' with '+': string concatenation is strictly typed (only string + string is allowed) — use a template string (`${{...}}`) or convert explicitly (Convert::*ToStr)", left, right),
            SemaError::GenericArityMismatch { name, expected_min, expected_max, found, .. } =>
                if expected_min == expected_max {
                    format!("generic '{}' expects {} type argument(s), {} provided", name, expected_min, found)
                } else {
                    format!("generic '{}' expects between {} and {} type argument(s), {} provided", name, expected_min, expected_max, found)
                },
            SemaError::ThreadAlreadyFinalized { name, .. } =>
                format!("'{}' was already '.join()' or '.detach()' — calling either a second time would use a native handle already reclaimed", name),
            SemaError::OnFilterClassNotFound { name, .. } =>
                format!("'{}' is not a known class — this 'on e is {}' handler would never match anything", name, name),
            SemaError::CatchAllNotLast { .. } =>
                "a catch-all 'on' handler (without 'is') must be the last one in this try/on chain — handlers after it would never be reached".into(),
            SemaError::ResourceAlreadyFinalized { name, class_name, method, .. } =>
                format!("'{}' ('{}') was already '.{}()' — calling it a second time would use a native handle already reclaimed", name, class_name, method),
            SemaError::MessageNotNameable { name, .. } =>
                format!("'{}': type 'message<T>' cannot be named — it is valid only as the declared return type of a function/method containing 'emit', never in a 'var'/'scoped'/'consumed' declaration", name),
            SemaError::MessageAsParamType { name, .. } =>
                format!("parameter '{}': type 'message<T>' cannot be used as a parameter type — it is return-type-only", name),
            SemaError::MessageReturnWithoutEmit { name, .. } =>
                format!("'{}' declares return type 'message<T>' but its body contains no reachable 'emit' — 'message<T>' is only valid as the return type of a function/method that actually emits", name),
            SemaError::EmitOutsideMessageFunction { .. } =>
                "'emit' is only valid inside a function/method whose declared return type is 'message<T>'".into(),
            SemaError::MessageUnsafeScalarConsumption { name, .. } =>
                format!("'{}()' returns 'message<T>' with an 'emit' reachable inside a loop — the compiler cannot prove that at most one value is ever produced, so it cannot be consumed directly as a scalar here; use 'for x in {}()' or 'Array::fromMessage({}())' instead", name, name, name),
            SemaError::IncDecInvalidType { found, .. } =>
                format!("'++'/'--' require an 'int' or 'float' target, found '{}'", found),
            SemaError::MethodCallOnVoid { method, .. } =>
                format!("cannot call '.{}(...)' — the receiver's type is 'void' (likely the return value of a preceding chained call); a method that returns 'void' cannot be chained, since there is nothing to call '.{}(...)' on", method, method),
            SemaError::MethodCallOnNonClass { type_name, method, available, .. } =>
                if available.is_empty() {
                    format!("cannot call '.{}(...)' — the receiver's type is '{}', which has no methods", method, type_name)
                } else {
                    format!("cannot call '.{}(...)' — the receiver's type is '{}', whose only methods are the conversions: {}", method, type_name, available.join(", "))
                },
            SemaError::InterfaceNoWiring { name, .. } =>
                format!("interface '{}' cannot be constructed or have a static method called on it directly: it has no 'wiring' declaration — add at least one 'wiring <Class>' inside the interface, or use a concrete implementing class directly", name),
            SemaError::ResolveOnNonResolvable { found, .. } =>
                format!("'resolve' expects a 'Resolvable<T>' expression (the result of calling an 'async' function/method), found '{}'", found),
            SemaError::AsyncReturnsResolvable { name, .. } =>
                format!("'{}' is 'async' and declares 'Resolvable<T>' as its own return type — an 'async' function/method already wraps its declared return type in 'Resolvable<T>' automatically at the call site; declare the real return type here instead (e.g. 'string', not 'Resolvable<string>')", name),
            SemaError::NamedArgMixed { callee, .. } =>
                format!("call to '{}' mixes positional and named arguments — a call is either fully positional or fully named", callee),
            SemaError::NamedArgUnknown { callee, name, valid, .. } =>
                if valid.is_empty() {
                    format!("'{}' has no parameter named '{}' — it takes no nameable parameter", callee, name)
                } else {
                    format!("'{}' has no parameter named '{}' — valid names: {}", callee, name, valid.join(", "))
                },
            SemaError::NamedArgDuplicate { callee, name, .. } =>
                format!("argument '{}' is provided twice in this call to '{}'", name, callee),
            SemaError::NamedArgVariadic { callee, name, .. } =>
                format!("parameter '{}' of '{}' is variadic and can only be passed positionally", name, callee),
            SemaError::NamedArgMissing { callee, name, .. } =>
                format!("missing argument '{}' in this named call to '{}' — it has no default value", name, callee),
            SemaError::NamedArgUnresolved { name, .. } =>
                format!("named argument '{}' cannot be used here: the parameter names of the called target are not statically known (e.g. a call through a 'Function<...>' value) — pass the arguments positionally", name),
            SemaError::FieldNotAccessible { class, field, protected, .. } =>
                if *protected {
                    format!("field '{}' of '{}' is protected — it is only accessible from '{}' and the classes/structs that extend it", field, class, class)
                } else {
                    format!("field '{}' of '{}' is private — it is only accessible from inside '{}' (expose it through a public method)", field, class, class)
                },
            SemaError::ArgsWithoutConstructor { class, found, .. } =>
                format!("'{}' has no init() — 'use {}()' takes no arguments, {} provided (declare init(...) that receives them — for an exception: init(message:string, code:int) {{ parent::init(message, code) }} — or use a struct for a constructor from its fields)", class, class, found),
            SemaError::ArithmeticOnNonNumeric { op, operand, .. } =>
                format!("operator '{}' cannot be applied to '{}' — only int, float and mixed support it ('+' also concatenates strings, '-=' removes occurrences from a string)", op, operand),
            SemaError::FieldCalledAsMethod { class, field, .. } =>
                format!("'{}' is a field of '{}', not a method — write '.{}' without parentheses", field, class, field),
            SemaError::ClassConstNotConstant { class, name, .. } =>
                format!("value of class constant '{}::{}' must be known at compile time — a literal, possibly negated or combined with +, -, *, /, % (e.g. '-273', '60 * 1000'); use a static method for a computed value", class, name),
            SemaError::GlobalConstNotConstant { name, .. } =>
                format!("value of global constant '{}' must be known at compile time — a literal, possibly negated or combined with +, -, *, /, % (e.g. '-273', '60 * 1000'); use a function for a computed value", name),
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
    /// `scoped`/`consumed` ressource (Mutex/SQLite/MySQL/MariaDB/
    /// HTTPRequest/HTTPResponse/Thread) encore ouverte au moment d'un
    /// `raise` qui n'est pas localement rattrapé par un `try` dans le même
    /// bloc — voir `crate::sema::resource_raise` et
    /// docs/roadmap.d/exceptions-setjmp-longjmp-dette.md. `longjmp` saute
    /// par-dessus la finalisation automatique de fin de bloc : fuite ou
    /// deadlock permanent, pas de corruption.
    ScopedResourceRaiseLeak { name: String, class_name: String, span: Span },
}

impl SemaWarning {
    pub fn span(&self) -> &Span {
        match self {
            SemaWarning::UnusedVariable { span, .. } => span,
            SemaWarning::MixedLocalVariable { span, .. } => span,
            SemaWarning::VariadicMixed { span, .. } => span,
            SemaWarning::ScopedResourceRaiseLeak { span, .. } => span,
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
            SemaWarning::ScopedResourceRaiseLeak { name, class_name, .. } =>
                format!("'{}' ('{}') is a 'scoped'/'consumed' resource still open when a 'raise' later in this block is not locally caught — its 'longjmp' skips this block's normal cleanup, leaking '{}' (or leaving a Mutex locked forever) — finalize it ('.destroy()'/'.close()'/'.join()'/'.detach()') before that 'raise', or wrap the risky code in a local 'try'/'on'", name, class_name, name),
        }
    }
}

impl fmt::Display for SemaWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.span(), self.message())
    }
}
