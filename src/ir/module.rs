use std::collections::HashMap;
use crate::ir::func::IrFunction;
use crate::ir::types::IrType;
use crate::ast::Literal;

// ─────────────────────────────────────────────────────────────────────────────
// IrModule — représentation complète d'un programme compilé
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
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
    /// Héritage : class_name → parent_name
    pub class_parents: HashMap<String, String>,
    /// Types des paramètres du constructeur : class_name → Vec<IrType>
    pub ctor_param_types: HashMap<String, Vec<IrType>>,
    /// Constantes de classes : "ClassName__NAME" → (IrType, Literal)
    pub class_consts: HashMap<String, (IrType, Literal)>,
    /// Compteur pour nommer les closures anonymes (__anon_0, __anon_1, ...)
    pub anon_counter: usize,
}

#[derive(Debug, Clone)]
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
