use std::collections::{HashMap, HashSet};
use crate::parsing::ast::{ClassDecl, ClassMember, Type, VarKind};
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Scope lexical empilé
// ─────────────────────────────────────────────────────────────────────────────

/// Un binding local (variable ou paramètre)
#[derive(Debug, Clone)]
pub struct LocalBinding {
    pub ty:      Type,
    pub mutable: bool,
    /// Déclaré à cette position (pour les warnings)
    pub span:    Span,
    /// Marqué vrai dès qu'on lit la variable
    pub used:    bool,
    /// Paramètre de fonction → pas de warning unused
    pub is_param: bool,
    /// `var` (défaut, pas de propriété) / `scoped` / `consumed`. Les
    /// paramètres et `const` sont toujours `Var` — ils ne sont pas
    /// possédés par ce mécanisme.
    pub kind: VarKind,
    /// Pour `kind == Consumed` seulement : position de sa première
    /// utilisation, une fois qu'elle a eu lieu (elle est détruite juste
    /// après — toute utilisation suivante est une erreur de compilation).
    pub consumed_used_at: Option<Span>,
    /// Pour une `scoped`/`consumed` ressource (`Thread`, `Mutex`, `SQLite`,
    /// `MySQL`/`MariaDB`) : vrai dès que sa méthode de finalisation manuelle
    /// (`.join()`/`.detach()` pour `Thread`, `.destroy()`/`.close()` pour les
    /// autres) a été appelée dessus — voir `mark_resource_finalized`. Pour
    /// `Thread` spécifiquement, aussi vérifié à la sortie du bloc (voir
    /// `OwnershipClass::Thread`) : une `Thread` qui l'atteint sans jamais
    /// avoir été finalisée est E19, pas ce champ.
    pub resource_finalized: bool,
    /// Pour un `var`/`const` de type ressource (`Mutex`/`SQLite`/`MySQL`/
    /// `MariaDB`) seulement : vrai si `crate::sema::escape::var_never_escapes`
    /// a prouvé, à la déclaration, qu'il ne s'échappe jamais (jamais retourné,
    /// réaffecté, ou passé en argument) — voir `pop_scope` : un `var`/`const`
    /// ressource qui remplit CETTE condition ET n'est jamais finalisé
    /// manuellement (`resource_finalized`) fuit son handle natif pour
    /// toujours, puisque ni `var` ni `const` ne libèrent jamais rien (à la
    /// différence de `scoped`/`consumed`, qui finalisent automatiquement en
    /// fin de bloc — voir `drop_func_for`). `false` par défaut : tant que ça
    /// n'a pas été prouvé contenu, on suppose prudemment qu'il pourrait
    /// s'échapper (aucun faux positif possible, voir
    /// docs/roadmap.d/memoire-documentation-diagnostics.md).
    pub resource_contained: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Propriété (`scoped`/`consumed`) — classification des types possédables
// ─────────────────────────────────────────────────────────────────────────────

/// Ce qui se passe quand une `scoped`/`consumed` de ce type s'échappe de son
/// bloc (affectation, `return`, argument d'appel), et comment elle est
/// détruite à son point de destruction. Voir le plan "Gestion de propriété
/// des variables" pour la justification de cette classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipClass {
    /// `string`/`array<T>`/`map<K,V>` — clonée automatiquement à
    /// l'échappement. Pour `string` spécifiquement : le runtime distingue
    /// désormais un littéral (`"foo"`, tag `TAG_STRING`, `.rodata`, jamais
    /// libéré ni cloné — aliasé directement, toujours sûr car immuable et
    /// éternel) d'une allocation tas réelle (tag `TAG_STRING_OWNED`, posée
    /// par `alloc_str` — concaténation, `String::*`, lecture fichier...),
    /// via `__value_free`/`__value_clone` (`runtime/src/lib.rs`). Avant ce
    /// tag dédié, les deux étaient indiscernables et libérer une `scoped
    /// string` plantait dès qu'elle contenait un littéral — voir
    /// l'historique de `TAG_STRING_OWNED` dans `runtime/src/typecheck.rs`.
    Value,
    /// Classe ressource avec un destructeur natif réel (Mutex/SQLite/
    /// MySQL/MariaDB) — échappement interdit, un handle ne peut pas être
    /// cloné ni partagé sans rendre le concept absurde ou dangereux.
    Resource,
    /// `Thread` — même interdiction d'échappement que `Resource`, mais pas
    /// de destructeur synthétisé : `.join()` (bloquant) vs `.detach()`
    /// (tâche de fond) est un choix sémantique que le compilateur ne peut
    /// pas prendre à la place du développeur.
    Thread,
    /// Type non pris en charge par ce chantier — uniquement les primitifs
    /// (`int`/`float`/`bool` : rien à posséder) et le reste (Function,
    /// union, `mixed`...) désormais.
    Unsupported,
}

/// `Type::Named(n)` non-ressource est traitée comme `Value` (instance de
/// classe utilisateur — clonée à l'échappement, comme `array`/`map`) SANS
/// vérifier ici si `n` a réellement un destructeur généré : cette fonction
/// n'a pas accès à `program.classes`. La sema (permissive : aucune classe
/// n'a besoin d'être "reconnue" pour qu'un échappement soit autorisé) et le
/// lowering restent cohérents grâce à un seul point de vérité côté lowering
/// — `crate::lower::builder::class_ownership::has_generated_destructor` —
/// consulté avant tout appel `__free_<Classe>`/`__clone_<Classe>` : si `n`
/// n'est pas une classe utilisateur réelle (SDL/Tauri/Exception/toute autre
/// classe builtin, qui n'ont pas d'entrée dans `class_field_types`), aucune
/// fonction n'est appelée — comportement identique à `var` (aucune
/// destruction, aucun clonage), pas de plantage ni de symbole manquant.
pub fn ownership_class(ty: &Type) -> OwnershipClass {
    match ty {
        Type::String | Type::Array(_) | Type::Map(_, _) => OwnershipClass::Value,
        Type::Named(n) => match n.as_str() {
            "Mutex" | "SQLite" | "MySQL" | "MariaDB" | "HTTPRequest" | "HTTPResponse" => OwnershipClass::Resource,
            "Thread" => OwnershipClass::Thread,
            _ => OwnershipClass::Value,
        },
        _ => OwnershipClass::Unsupported,
    }
}

/// Calcule l'ensemble des classes utilisateur qui « contiennent » une
/// ressource native (`Mutex`/`SQLite`/`MySQL`/`MariaDB`/`HTTPRequest`/
/// `HTTPResponse`), directement (une `property` de ce type) ou
/// transitivement (une `property` dont la classe est elle-même dans cet
/// ensemble) — fixpoint jusqu'à stabilisation, `program.classes` étant de
/// taille finie et chaque itération ajoutant au moins un élément ou
/// s'arrêtant. `Thread` est délibérément exclu (même périmètre que E29 —
/// `ownership_class(ty) == OwnershipClass::Resource` — voir sa doc).
///
/// Une classe de cet ensemble DOIT être traitée comme
/// `OwnershipClass::Resource` partout où `ownership_class` est consulté pour
/// une décision d'échappement/de possession (voir `ownership_class_of`) :
/// son destructeur ferme désormais un handle natif (voir
/// `crate::lower::builder::class_ownership`), donc elle hérite des mêmes
/// règles qu'une ressource nue (E18 : ne peut pas s'échapper de son bloc
/// `scoped`/`consumed` ; E28 : un `var`/`const` prouvé non-échappant doit
/// être fermé manuellement, jamais auto-libéré silencieusement). Sans ça,
/// deux instances pourraient se retrouver à partager le même handle natif
/// (`__clone_<Classe>` copie un champ ressource tel quel, jamais dupliqué —
/// voir `FieldOwnership::Resource`) et l'une fermerait la ressource sous le
/// nez de l'autre : double-free / use-after-close silencieux.
pub fn compute_resource_classes(classes: &[ClassDecl]) -> HashSet<String> {
    let mut result: HashSet<String> = HashSet::new();
    loop {
        let mut changed = false;
        for class in classes {
            if result.contains(&class.name) {
                continue;
            }
            let contains_resource = class.members.iter().any(|m| match m {
                ClassMember::Field { ty, .. } => match ty {
                    Type::Named(n) => {
                        ownership_class(ty) == OwnershipClass::Resource || result.contains(n)
                    }
                    _ => false,
                },
                _ => false,
            });
            if contains_resource {
                result.insert(class.name.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    result
}

/// Comme `ownership_class`, mais en plus conscient des classes utilisateur
/// qui contiennent (directement ou transitivement) une ressource native
/// (`resource_classes`, voir `compute_resource_classes`) — celles-ci doivent
/// être traitées en `OwnershipClass::Resource`, pas `Value`, partout où une
/// décision d'échappement/de possession est prise. Un `Type::Named` absent
/// de `resource_classes` retombe sur `ownership_class` normal (classe
/// utilisateur ordinaire, builtin, ou primitif).
pub fn ownership_class_of(ty: &Type, resource_classes: &HashSet<String>) -> OwnershipClass {
    if let Type::Named(n) = ty {
        if resource_classes.contains(n) {
            return OwnershipClass::Resource;
        }
    }
    ownership_class(ty)
}

/// Nom du symbole runtime qui finalise une valeur du type ressource NATIF
/// nommé (`SQLite_close`, `Mutex_destroy`, ...) — `None` si `ty_name` n'est
/// pas l'un des types listés dans `ownership_class`. Point de vérité unique,
/// partagé par `lower::stmt::ownership::drop_func_for` (finalisation d'une
/// `scoped`/`consumed`/`var` ressource) et `lower::builder::class_ownership`
/// (finalisation d'un CHAMP ressource dans `__free_<Classe>`) — éviter deux
/// copies de ce mapping qui pourraient diverger silencieusement.
pub fn resource_closer_symbol(ty_name: &str) -> Option<&'static str> {
    match ty_name {
        "Mutex" => Some("Mutex_destroy"),
        "SQLite" => Some("SQLite_close"),
        "MySQL" => Some("MySQL_close"),
        "MariaDB" => Some("MariaDB_close"),
        "HTTPRequest" => Some("HTTPRequest_close"),
        "HTTPResponse" => Some("HTTPRequest_closeResponse"),
        _ => None,
    }
}

/// Variable non utilisée retournée par `pop_scope`.
pub struct UnusedVar {
    pub name: String,
    pub span: Span,
}

/// `scoped`/`consumed Thread` qui atteint la fin de son bloc sans avoir été
/// `.join()`e ni `.detach()`e — retourné par `pop_scope`.
pub struct UnfinalizedThread {
    pub name: String,
    pub span: Span,
}

/// `var`/`const` d'un type ressource (`Mutex`/`SQLite`/`MySQL`/`MariaDB`)
/// prouvé "contenu" (`resource_contained`, jamais échappé) qui atteint la fin
/// de son bloc sans jamais avoir été finalisé manuellement — un handle natif
/// qui fuit pour toujours, puisque `var`/`const` ne libèrent jamais rien
/// automatiquement (contrairement à `scoped`/`consumed`, voir
/// `OwnershipClass::Resource`/`drop_func_for`). Retourné par `pop_scope`.
pub struct UnclosedResourceVar {
    pub name: String,
    pub ty_name: String,
    pub span: Span,
}

/// Résultat du dépilement d'un scope : les variables inutilisées (warning
/// existant), les `Thread` `scoped`/`consumed` non finalisées, et les
/// `var`/`const` ressource qui fuient leur handle natif (les deux dernières :
/// nouvelles erreurs).
pub struct PoppedScope {
    pub unused: Vec<UnusedVar>,
    pub unfinalized_threads: Vec<UnfinalizedThread>,
    pub unclosed_resource_vars: Vec<UnclosedResourceVar>,
}

/// Pile de scopes lexicaux.
/// Le sommet (index 0) est le scope le plus interne.
#[derive(Debug, Default)]
pub struct ScopeStack {
    frames: Vec<HashMap<String, LocalBinding>>,
}

impl ScopeStack {
    pub fn push(&mut self) {
        self.frames.push(HashMap::new());
    }

    #[allow(dead_code)]
    pub fn pop(&mut self) {
        self.frames.pop();
    }

    /// Dépile le scope courant et retourne les variables non utilisées ainsi
    /// que les `Thread` `scoped`/`consumed` jamais `.join()`/`.detach()`.
    /// `resource_classes` (voir `compute_resource_classes`) permet de
    /// traiter une instance d'une classe utilisateur contenant une ressource
    /// exactement comme une ressource nue (E28 ci-dessous).
    pub fn pop_scope(&mut self, resource_classes: &HashSet<String>) -> PoppedScope {
        let frame = match self.frames.pop() {
            Some(f) => f,
            None    => return PoppedScope { unused: vec![], unfinalized_threads: vec![], unclosed_resource_vars: vec![] },
        };
        let mut unused: Vec<UnusedVar> = Vec::new();
        let mut unfinalized_threads: Vec<UnfinalizedThread> = Vec::new();
        let mut unclosed_resource_vars: Vec<UnclosedResourceVar> = Vec::new();
        for (name, b) in frame {
            if !b.used && !b.is_param {
                unused.push(UnusedVar { name: name.clone(), span: b.span.clone() });
            }
            if !b.is_param
                && matches!(b.kind, VarKind::Scoped | VarKind::Consumed)
                && ownership_class(&b.ty) == OwnershipClass::Thread
                && !b.resource_finalized
            {
                unfinalized_threads.push(UnfinalizedThread { name: name.clone(), span: b.span.clone() });
            }
            // `var`/`const` (jamais `scoped`/`consumed`, qui finalisent déjà
            // automatiquement en fin de bloc, voir `drop_func_for`) : un
            // handle ressource prouvé "contenu" et jamais finalisé
            // manuellement fuit pour toujours — voir
            // `LocalBinding::resource_contained`.
            if !b.is_param
                && b.kind == VarKind::Var
                && b.resource_contained
                && !b.resource_finalized
            {
                if let crate::parsing::ast::Type::Named(ty_name) = &b.ty {
                    if ownership_class_of(&b.ty, resource_classes) == OwnershipClass::Resource {
                        unclosed_resource_vars.push(UnclosedResourceVar {
                            name, ty_name: ty_name.clone(), span: b.span.clone(),
                        });
                    }
                }
            }
        }
        // Tri pour ordre déterministe (ligne, colonne)
        unused.sort_by_key(|u| (u.span.line, u.span.col));
        unfinalized_threads.sort_by_key(|u| (u.span.line, u.span.col));
        unclosed_resource_vars.sort_by_key(|u| (u.span.line, u.span.col));
        PoppedScope { unused, unfinalized_threads, unclosed_resource_vars }
    }

    /// Déclare un symbole dans le scope courant.
    /// Retourne `false` si le nom est déjà déclaré dans ce scope exact.
    pub fn declare(&mut self, name: String, binding: LocalBinding) -> bool {
        let top = self.frames.last_mut().expect("scope stack vide");
        if top.contains_key(&name) {
            return false;
        }
        top.insert(name, binding);
        true
    }

    /// Recherche en remontant la pile.
    pub fn lookup(&self, name: &str) -> Option<&LocalBinding> {
        for frame in self.frames.iter().rev() {
            if let Some(b) = frame.get(name) {
                return Some(b);
            }
        }
        None
    }

    /// Marque une variable comme utilisée (en remontant la pile), et fait
    /// respecter la règle "`consumed` utilisée au plus une fois" au passage.
    /// Retourne `Err(première_position)` si `name` désigne une `consumed`
    /// déjà utilisée avant `use_span` — le point d'usage originel, pour que
    /// l'appelant puisse citer les deux positions dans l'erreur.
    pub fn use_binding(&mut self, name: &str, use_span: &Span) -> Result<(), Span> {
        for frame in self.frames.iter_mut().rev() {
            if let Some(b) = frame.get_mut(name) {
                if b.kind == VarKind::Consumed {
                    if let Some(first) = &b.consumed_used_at {
                        return Err(first.clone());
                    }
                    b.consumed_used_at = Some(use_span.clone());
                }
                b.used = true;
                return Ok(());
            }
        }
        Ok(())
    }

    /// Marque une ressource (`Thread`/`Mutex`/`SQLite`/`MySQL`/`MariaDB`)
    /// comme finalisée manuellement (`.join()`/`.detach()`/`.destroy()`/
    /// `.close()` selon le type — voir l'appelant). Retourne `true` si elle
    /// l'était déjà — un second appel referait un `Box::from_raw`/une
    /// libération sur un pointeur déjà repris côté runtime, un use-after-free
    /// confirmé par reproduction pour chacun de ces types (abort pour
    /// `Thread`, SEGFAULT pour `Mutex`) — voir
    /// `docs/roadmap.d/memoire-double-free-et-fuites-scoped.md` et
    /// `docs/roadmap.d/memoire-documentation-diagnostics.md`.
    pub fn mark_resource_finalized(&mut self, name: &str) -> bool {
        for frame in self.frames.iter_mut().rev() {
            if let Some(b) = frame.get_mut(name) {
                let already = b.resource_finalized;
                b.resource_finalized = true;
                return already;
            }
        }
        false
    }

    /// Marque un `var`/`const` ressource comme prouvé "contenu" — voir
    /// `LocalBinding::resource_contained`. Cherche uniquement dans le frame
    /// COURANT (contrairement à `mark_resource_finalized`) : appelé juste
    /// après la déclaration, donc toujours dans le frame où `name` vient
    /// d'être inséré.
    pub fn mark_resource_contained(&mut self, name: &str) {
        if let Some(frame) = self.frames.last_mut() {
            if let Some(b) = frame.get_mut(name) {
                b.resource_contained = true;
            }
        }
    }
}
