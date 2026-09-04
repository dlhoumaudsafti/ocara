use std::collections::{HashMap, HashSet};
use crate::ir::func::IrFunction;
use crate::ir::types::IrType;
use crate::parsing::ast::{Literal, Type};

// ─────────────────────────────────────────────────────────────────────────────
// IrModule — représentation complète d'un programme compilé
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct IrModule {
    pub name:      String,
    pub source_file: String,
    pub functions: Vec<IrFunction>,
    /// Table des chaînes littérales (index → contenu)
    pub strings:   Vec<String>,
    /// Constantes globales (nom → valeur scalaire en bytes)
    pub globals:   Vec<IrGlobal>,
    /// Modules importés (ex: ["IO", "Array", "Math"])
    pub imports:   Vec<String>,
    /// Layout des classes : class_name → liste ordonnée (field_name, field_type)
    pub class_layouts: HashMap<String, Vec<(String, IrType)>>,
    /// Comme `class_layouts`, mais avec le vrai type AST de chaque champ
    /// (pas `IrType`, qui réduit string/array/map/instance à `Ptr`, tous
    /// indistinguables) — indispensable pour générer les destructeurs/
    /// clones réels des instances de classe utilisateur (`scoped`/
    /// `consumed MaClasse`, voir `src/lower/builder.d/class_ownership.rs`).
    /// UNIQUEMENT pour les classes utilisateur (`program.classes`) — sert
    /// aussi de test d'appartenance : une classe absente d'ici (builtin/
    /// opaque comme Mutex/SDL/Exception) n'a PAS de `__free_<Classe>`/
    /// `__clone_<Classe>` généré, donc ne doit jamais être traitée comme un
    /// objet possédable (voir `class_ownership::has_generated_destructor`).
    pub class_field_types: HashMap<String, Vec<(String, Type)>>,
    /// Champs de type map<K,V> par classe (hérités inclus) : class_name → noms de
    /// champs. `class_layouts` réduit tout à IrType::Ptr (map/array/string
    /// indistinguables) — indispensable pour que `self.champMap[clé] = v` émette
    /// `__map_set` plutôt que `__array_set` (voir Expr::Index dans lower.rs et
    /// assignments.rs : la distinction map/array pour un accès par index ne peut
    /// se faire que via ce genre de méta-info, jamais via IrType seul).
    pub class_map_fields: HashMap<String, HashSet<String>>,
    /// Héritage : class_name → parent_name
    pub class_parents: HashMap<String, String>,
    /// Types des paramètres du constructeur : class_name → Vec<IrType>
    pub ctor_param_types: HashMap<String, Vec<IrType>>,
    /// Constantes de classes : "ClassName__NAME" → (IrType, Literal)
    pub class_consts: HashMap<String, (IrType, Literal)>,
    /// Compteur pour nommer les closures anonymes (__anon_0, __anon_1, ...)
    pub anon_counter: usize,
    /// Compteur pour nommer les fonctions try/handler (__try_body_0, __try_handler_0, ...).
    /// Doit être un compteur dédié, PAS `functions.len()` : un `try` imbriqué dans le
    /// corps d'un autre `try` est lowered (et ajoute ses propres fonctions au module)
    /// AVANT que le `try` englobant n'ajoute les siennes (voir lower_try) — dériver
    /// l'id de `functions.len()` fait alors lire la même longueur pour l'englobant et
    /// l'imbriqué, produisant deux fonctions au même nom (collision de signature au
    /// codegen). Même patron que `anon_counter` ci-dessus, qui n'a pas ce problème.
    pub try_counter: usize,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct IrGlobal {
    pub name:  String,
    pub bytes: Vec<u8>,
}

impl IrModule {
    pub fn new(name: impl Into<String>) -> Self {
        Self { 
            name: name.into(), 
            source_file: String::new(),
            ..Default::default() 
        }
    }

    /// Enregistre une chaîne littérale et retourne son index
    pub fn intern_string(&mut self, s: &str) -> u32 {
        if let Some(i) = self.strings.iter().position(|x| x == s) {
            return i as u32;
        }
        let i = self.strings.len() as u32;
        self.strings.push(s.to_string());
        i
    }

    pub fn add_function(&mut self, func: IrFunction) {
        self.functions.push(func);
    }

    pub fn add_global(&mut self, global: IrGlobal) {
        self.globals.push(global);
    }
}
