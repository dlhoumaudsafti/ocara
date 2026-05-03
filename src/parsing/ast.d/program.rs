/// Programme (racine de l'AST)

use super::imports::{ImportDecl, ConstDecl};
use super::classes::{ClassDecl, ModuleDecl};
use super::generics::GenericDecl;
use super::interfaces::InterfaceDecl;
use super::enums::EnumDecl;
use super::functions::FuncDecl;
use super::runtime::{RuntimeImport, RuntimeBlock};

// ─────────────────────────────────────────────────────────────────────────────
// Programme (racine de l'AST)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub namespace:  Option<String>, // None ou "." = racine, "classes" = namespace classes, etc.
    pub imports:    Vec<ImportDecl>,
    pub runtime_imports: Vec<RuntimeImport>,
    pub runtime_blocks:  Vec<RuntimeBlock>,
    pub consts:     Vec<ConstDecl>,
    pub modules:    Vec<ModuleDecl>,
    pub enums:      Vec<EnumDecl>,
    pub classes:    Vec<ClassDecl>,
    pub generics:   Vec<GenericDecl>,
    pub interfaces: Vec<InterfaceDecl>,
    pub functions:  Vec<FuncDecl>,
}

impl Program {
    pub fn new() -> Self {
        Self {
            namespace:  None,
            imports:    Vec::new(),
            runtime_imports: Vec::new(),
            runtime_blocks:  Vec::new(),
            consts:     Vec::new(),
            modules:    Vec::new(),
            enums:      Vec::new(),
            classes:    Vec::new(),
            generics:   Vec::new(),
            interfaces: Vec::new(),
            functions:  Vec::new(),
        }
    }
}
