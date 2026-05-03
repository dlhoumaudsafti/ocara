/// Définitions des types pour la table des symboles
use std::collections::HashMap;
use crate::parsing::ast::{Type, Visibility, TypeParam};

// ─────────────────────────────────────────────────────────────────────────────
// Descripteurs de symboles
// ─────────────────────────────────────────────────────────────────────────────

/// Signature d'une fonction (méthode ou fonction libre)
#[derive(Debug, Clone)]
pub struct FuncSig {
    pub params:    Vec<(String, Type)>,
    pub ret_ty:    Type,
    pub is_static: bool,
    pub is_async:  bool,
    pub has_variadic: bool,  // true si le dernier paramètre est variadic
    pub fixed_params_count: usize,  // nombre de paramètres fixes (avant variadic)
    pub required_params_count: usize,  // nombre de paramètres obligatoires (sans default_value)
}

/// Descripteur d'un champ de classe
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub ty:      Type,
    pub mutable: bool,
    pub vis:     Visibility,
}

/// Descripteur complet d'une classe enregistrée
#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub extends:      Option<String>,
    pub implements:   Vec<String>,
    pub fields:       HashMap<String, FieldInfo>,
    pub methods:      HashMap<String, FuncSig>,
    pub class_consts: HashMap<String, (Type, Visibility)>,
    /// Vrai si la classe est issue d'un import non résolu (accès opaques autorisés)
    pub is_opaque:    bool,
}

/// Descripteur complet d'un module (mixin)
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub fields:       HashMap<String, FieldInfo>,
    pub methods:      HashMap<String, FuncSig>,
    pub class_consts: HashMap<String, (Type, Visibility)>,
}

/// Descripteur d'une interface enregistrée
#[derive(Debug, Clone)]
pub struct InterfaceInfo {
    pub methods: HashMap<String, FuncSig>,
}

/// Descripteur d'un import
#[derive(Debug, Clone)]
pub struct ImportInfo {
    pub path:  Vec<String>,
    pub alias: Option<String>,
}

/// Descripteur d'un enum
#[derive(Debug, Clone)]
pub struct EnumInfo {
    /// Variantes avec leur valeur int
    pub variants: HashMap<String, i64>,
}

/// Descripteur d'un générique enregistré
#[derive(Debug, Clone)]
pub struct GenericInfo {
    pub type_params: Vec<TypeParam>,
    pub extends:     Option<String>,
    pub extends_args: Vec<Type>,
    pub implements:  Vec<String>,
    pub fields:      HashMap<String, FieldInfo>,
    pub methods:     HashMap<String, FuncSig>,
    pub class_consts: HashMap<String, (Type, Visibility)>,
}

// ─────────────────────────────────────────────────────────────────────────────
// SymbolTable
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct SymbolTable {
    pub functions:  HashMap<String, FuncSig>,
    pub modules:    HashMap<String, ModuleInfo>,
    pub classes:    HashMap<String, ClassInfo>,
    pub interfaces: HashMap<String, InterfaceInfo>,
    pub enums:      HashMap<String, EnumInfo>,
    pub generics:   HashMap<String, GenericInfo>,
    pub consts:     HashMap<String, Type>,
    pub imports:    Vec<ImportInfo>,
}
