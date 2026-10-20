//! Conflits d'un renommage : le nouveau nom ne doit pas déjà être déclaré
//! là où la déclaration renommée est visible. Sans ce contrôle, renommer une
//! locale `count` en `total` (déjà déclarée) ou en `IO` était accepté, puis
//! le programme ne compilait plus (`duplicate symbol`).
//!
//! Volontairement prudent : tous les noms de la fonction englobante (blocs
//! frères compris), les membres de sa classe et les déclarations de premier
//! niveau du fichier comptent.

use std::collections::HashSet;

use crate::parsing::ast::{Block, ClassMember, FuncDecl, Program, Stmt};

/// Message de refus si `new_name` est déjà pris pour une déclaration de la
/// ligne `line` (1-based) de `own`.
pub fn conflict(own: &Program, line: usize, new_name: &str) -> Option<String> {
    if crate::builtins::builtin_class(new_name).is_some() {
        return Some(format!("« {} » est une classe builtin", new_name));
    }
    let taken = names_visible_at(own, line);
    taken.contains(new_name).then(|| format!("« {} » est déjà déclaré à cet endroit", new_name))
}

fn names_visible_at(own: &Program, line: usize) -> HashSet<String> {
    let mut names: HashSet<String> = top_level_names(own);
    if let Some(func) = enclosing_function(own, line) {
        names.extend(func.params.iter().map(|p| p.name.clone()));
        collect_block(&func.body, &mut names);
    }
    if let Some(members) = enclosing_class_members(own, line) {
        names.extend(members.iter().filter_map(member_name));
    }
    names
}

fn top_level_names(own: &Program) -> HashSet<String> {
    own.classes.iter().map(|c| c.name.clone())
        .chain(own.interfaces.iter().map(|i| i.name.clone()))
        .chain(own.generics.iter().map(|g| g.name.clone()))
        .chain(own.modules.iter().map(|m| m.name.clone()))
        .chain(own.enums.iter().map(|e| e.name.clone()))
        .chain(own.functions.iter().map(|f| f.name.clone()))
        .chain(own.consts.iter().map(|c| c.name.clone()))
        .collect()
}

/// Fonction ou méthode qui commence le plus près avant `line`.
fn enclosing_function(own: &Program, line: usize) -> Option<&FuncDecl> {
    let methods = own.classes.iter().flat_map(|c| &c.members).filter_map(|m| match m {
        ClassMember::Method { decl, .. } => Some(decl),
        _ => None,
    });
    own.functions.iter().chain(methods)
        .filter(|f| f.span.line <= line)
        .max_by_key(|f| f.span.line)
}

fn enclosing_class_members(own: &Program, line: usize) -> Option<&Vec<ClassMember>> {
    own.classes.iter()
        .filter(|c| c.span.line <= line)
        .max_by_key(|c| c.span.line)
        .map(|c| &c.members)
}

fn member_name(member: &ClassMember) -> Option<String> {
    match member {
        ClassMember::Field { name, .. } | ClassMember::Const { name, .. } => Some(name.clone()),
        ClassMember::Method { decl, .. } => Some(decl.name.clone()),
        ClassMember::Constructor { .. } => None,
    }
}

fn collect_block(block: &Block, names: &mut HashSet<String>) {
    for stmt in &block.stmts {
        collect_stmt(stmt, names);
    }
}

fn collect_stmt(stmt: &Stmt, names: &mut HashSet<String>) {
    match stmt {
        Stmt::Var { name, .. } | Stmt::Const { name, .. } => { names.insert(name.clone()); }
        Stmt::ForIn { var, body, .. } => {
            names.insert(var.clone());
            collect_block(body, names);
        }
        Stmt::ForMap { key, value, body, .. } => {
            names.insert(key.clone());
            names.insert(value.clone());
            collect_block(body, names);
        }
        Stmt::If { then_block, elseif, else_block, .. } => {
            collect_block(then_block, names);
            for (_, b) in elseif {
                collect_block(b, names);
            }
            if let Some(b) = else_block {
                collect_block(b, names);
            }
        }
        Stmt::While { body, .. } => collect_block(body, names),
        Stmt::Switch { cases, default, .. } => {
            for c in cases {
                collect_block(&c.body, names);
            }
            if let Some(b) = default {
                collect_block(b, names);
            }
        }
        Stmt::Try { body, handlers, .. } => {
            collect_block(body, names);
            for h in handlers {
                names.insert(h.binding.clone());
                collect_block(&h.body, names);
            }
        }
        _ => {}
    }
}
