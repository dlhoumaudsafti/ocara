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
    /// `array<T>`/`map<K,V>` — clonée automatiquement à l'échappement.
    /// NOTE : `string` n'est PAS ici malgré son type tas — un littéral
    /// (`"foo"`) est compilé en une constante `.rodata` portant le même
    /// header `TAG_STRING` qu'une vraie allocation tas (voir
    /// `src/codegen/emit.d/instructions.d/constants.rs::Inst::ConstStr`),
    /// donc indiscernable au runtime d'une string réellement possédée —
    /// tenter de la libérer plante (`dealloc` sur une adresse jamais
    /// allouée par l'allocateur). Distinguer les deux nécessite soit un tag
    /// runtime dédié, soit une preuve statique à la déclaration ; ni l'un
    /// ni l'autre n'existe encore, donc `string` reste `Unsupported` pour
    /// ce chantier (se comporte comme `var`) jusqu'à ce que ce soit résolu.
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
    /// Type non pris en charge par ce chantier — primitif (rien à
    /// posséder), `string` (voir `Value` ci-dessus), SDL/Tauri (cycle de
    /// vie "tout le process", incompatible avec une portée de bloc), ou
    /// instance de classe utilisateur (pas de clonage/destructeur
    /// générique pour l'instant).
    Unsupported,
}

pub fn ownership_class(ty: &Type) -> OwnershipClass {
    match ty {
        Type::Array(_) | Type::Map(_, _) => OwnershipClass::Value,
        Type::Named(n) => match n.as_str() {
            "Mutex" | "SQLite" | "MySQL" | "MariaDB" => OwnershipClass::Resource,
            "Thread" => OwnershipClass::Thread,
            _ => OwnershipClass::Unsupported,
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
    /// que les `Thread` `scoped`/`consumed` jamais `.join()`ées/`.detach()`ées.
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
