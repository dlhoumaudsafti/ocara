use std::collections::HashMap;
use crate::parsing::ast::{Type, VarKind};
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
    /// Pour une `scoped`/`consumed Thread` uniquement : vrai dès que
    /// `.join()` ou `.detach()` a été appelé dessus. Vérifié à la sortie du
    /// bloc — voir `OwnershipClass::Thread`.
    pub thread_finalized: bool,
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
            "Mutex" | "SQLite" | "MySQL" | "MariaDB" => OwnershipClass::Resource,
            "Thread" => OwnershipClass::Thread,
            _ => OwnershipClass::Value,
        },
        _ => OwnershipClass::Unsupported,
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

/// Résultat du dépilement d'un scope : à la fois les variables inutilisées
/// (warning existant) et les `Thread` `scoped`/`consumed` non finalisées
/// (nouvelle erreur — voir `OwnershipClass::Thread`).
pub struct PoppedScope {
    pub unused: Vec<UnusedVar>,
    pub unfinalized_threads: Vec<UnfinalizedThread>,
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
    pub fn pop_scope(&mut self) -> PoppedScope {
        let frame = match self.frames.pop() {
            Some(f) => f,
            None    => return PoppedScope { unused: vec![], unfinalized_threads: vec![] },
        };
        let mut unused: Vec<UnusedVar> = Vec::new();
        let mut unfinalized_threads: Vec<UnfinalizedThread> = Vec::new();
        for (name, b) in frame {
            if !b.used && !b.is_param {
                unused.push(UnusedVar { name: name.clone(), span: b.span.clone() });
            }
            if !b.is_param
                && matches!(b.kind, VarKind::Scoped | VarKind::Consumed)
                && ownership_class(&b.ty) == OwnershipClass::Thread
                && !b.thread_finalized
            {
                unfinalized_threads.push(UnfinalizedThread { name, span: b.span.clone() });
            }
        }
        // Tri pour ordre déterministe (ligne, colonne)
        unused.sort_by_key(|u| (u.span.line, u.span.col));
        unfinalized_threads.sort_by_key(|u| (u.span.line, u.span.col));
        PoppedScope { unused, unfinalized_threads }
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

    /// Marque une `Thread` comme finalisée (`.join()`/`.detach()` appelé).
    pub fn mark_thread_finalized(&mut self, name: &str) {
        for frame in self.frames.iter_mut().rev() {
            if let Some(b) = frame.get_mut(name) {
                b.thread_finalized = true;
                return;
            }
        }
    }
}
