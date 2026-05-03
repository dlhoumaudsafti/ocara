/// Déclarations de classes et modules (mixins)

use super::types::{Type, Visibility};
use super::expressions::Expr;
use super::statements::Block;
use super::params::Param;
use super::functions::FuncDecl;
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Membre d'une classe
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember {
    Field {
        vis:     Visibility,
        mutable: bool,
        name:    String,
        ty:      Type,
        span:    Span,
    },
    Const {
        vis:   Visibility,
        name:  String,
        ty:    Type,
        value: Expr,
        span:  Span,
    },
    Method {
        vis:       Visibility,
        is_static: bool,
        decl:      FuncDecl,
        span:      Span,
    },
    Constructor {
        params: Vec<Param>,
        body:   Block,
        span:   Span,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Déclaration de classe
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDecl {
    pub name:       String,
    pub extends:    Option<String>,
    pub modules:    Vec<String>,
    pub implements: Vec<String>,
    pub members:    Vec<ClassMember>,
    pub span:       Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Déclaration de module (mixin)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDecl {
    pub name:    String,
    pub members: Vec<ClassMember>,
    pub span:    Span,
}
