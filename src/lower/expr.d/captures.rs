/// Analyse des captures pour les closures

use std::collections::{HashMap, HashSet};
use crate::parsing::ast::*;
use crate::ir::inst::Value;
use crate::ir::types::IrType;

/// Retourne la liste des variables locales du scope englobant référencées dans `body`,
/// en excluant les paramètres propres de la closure.
pub fn collect_captures(
    body:        &Block,
    param_names: &HashSet<String>,
    locals:      &HashMap<String, (Value, IrType, bool)>,
) -> Vec<(String, IrType)> {
    let mut caps: Vec<(String, IrType)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    walk_block_caps(body, param_names, locals, &mut caps, &mut seen);
    caps
}

fn walk_block_caps(b: &Block, p: &HashSet<String>, l: &HashMap<String, (Value, IrType, bool)>, caps: &mut Vec<(String, IrType)>, seen: &mut HashSet<String>) {
    for stmt in &b.stmts { walk_stmt_caps(stmt, p, l, caps, seen); }
}

fn walk_stmt_caps(stmt: &Stmt, p: &HashSet<String>, l: &HashMap<String, (Value, IrType, bool)>, caps: &mut Vec<(String, IrType)>, seen: &mut HashSet<String>) {
    match stmt {
        Stmt::Var   { value, .. }     => walk_expr_caps(value, p, l, caps, seen),
        Stmt::Const { value, .. }     => walk_expr_caps(value, p, l, caps, seen),
        Stmt::Expr(e)                 => walk_expr_caps(e, p, l, caps, seen),
        Stmt::Assign { target, value, .. } => {
            walk_expr_caps(target, p, l, caps, seen);
            walk_expr_caps(value,  p, l, caps, seen);
        }
        Stmt::Return { value: Some(e), .. } => walk_expr_caps(e, p, l, caps, seen),
        Stmt::Return { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {}
        Stmt::If { condition, then_block, elseif, else_block, .. } => {
            walk_expr_caps(condition, p, l, caps, seen);
            walk_block_caps(then_block, p, l, caps, seen);
            for (c, blk) in elseif { walk_expr_caps(c, p, l, caps, seen); walk_block_caps(blk, p, l, caps, seen); }
            if let Some(blk) = else_block { walk_block_caps(blk, p, l, caps, seen); }
        }
        Stmt::While { condition, body, .. } => {
            walk_expr_caps(condition, p, l, caps, seen);
            walk_block_caps(body, p, l, caps, seen);
        }
        Stmt::ForIn { iter, body, .. } => {
            walk_expr_caps(iter, p, l, caps, seen);
            walk_block_caps(body, p, l, caps, seen);
        }
        Stmt::ForMap { iter, body, .. } => {
            walk_expr_caps(iter, p, l, caps, seen);
            walk_block_caps(body, p, l, caps, seen);
        }
        Stmt::Switch { subject, cases, default, .. } => {
            walk_expr_caps(subject, p, l, caps, seen);
            for c in cases { walk_block_caps(&c.body, p, l, caps, seen); }
            if let Some(blk) = default { walk_block_caps(blk, p, l, caps, seen); }
        }
        Stmt::Try { body, handlers, .. } => {
            walk_block_caps(body, p, l, caps, seen);
            for h in handlers { walk_block_caps(&h.body, p, l, caps, seen); }
        }
        Stmt::Raise { value, .. } => walk_expr_caps(value, p, l, caps, seen),
    }
}

fn walk_expr_caps(expr: &Expr, p: &HashSet<String>, l: &HashMap<String, (Value, IrType, bool)>, caps: &mut Vec<(String, IrType)>, seen: &mut HashSet<String>) {
    match expr {
        Expr::Ident(name, _) => {
            if !p.contains(name.as_str()) && !seen.contains(name.as_str()) {
                if let Some((_, ty, _)) = l.get(name.as_str()) {
                    caps.push((name.clone(), ty.clone()));
                    seen.insert(name.clone());
                }
            }
        }
        Expr::SelfExpr(_) => {
            let key = "self";
            if !seen.contains(key) {
                if let Some((_, ty, _)) = l.get(key) {
                    caps.push((key.to_string(), ty.clone()));
                    seen.insert(key.to_string());
                }
            }
        }
        Expr::Binary { left, right, .. } => { walk_expr_caps(left, p, l, caps, seen); walk_expr_caps(right, p, l, caps, seen); }
        Expr::Unary  { operand, .. }     => walk_expr_caps(operand, p, l, caps, seen),
        Expr::Field  { object, .. }      => walk_expr_caps(object, p, l, caps, seen),
        Expr::Call   { callee, args, .. } => { walk_expr_caps(callee, p, l, caps, seen); for a in args { walk_expr_caps(a, p, l, caps, seen); } }
        Expr::StaticCall { args, .. }    => { for a in args { walk_expr_caps(a, p, l, caps, seen); } }
        Expr::New    { args, .. }        => { for a in args { walk_expr_caps(a, p, l, caps, seen); } }
        Expr::Index  { object, index, ..} => { walk_expr_caps(object, p, l, caps, seen); walk_expr_caps(index, p, l, caps, seen); }
        Expr::Range  { start, end, .. }  => { walk_expr_caps(start, p, l, caps, seen); walk_expr_caps(end, p, l, caps, seen); }
        Expr::Array  { elements, .. }    => { for e in elements { walk_expr_caps(e, p, l, caps, seen); } }
        Expr::Map    { entries, .. }     => { for (k, v) in entries { walk_expr_caps(k, p, l, caps, seen); walk_expr_caps(v, p, l, caps, seen); } }
        Expr::Template { parts, .. }     => { for part in parts { if let TemplatePartExpr::Expr(e) = part { walk_expr_caps(e, p, l, caps, seen); } } }
        Expr::Match  { subject, arms, ..} => { walk_expr_caps(subject, p, l, caps, seen); for arm in arms { walk_expr_caps(&arm.body, p, l, caps, seen); } }
        Expr::IsCheck { expr, .. }       => walk_expr_caps(expr, p, l, caps, seen),
        Expr::Resolve { expr, .. }        => walk_expr_caps(expr, p, l, caps, seen),
        // Ne pas descendre dans les nameless imbriquées (elles ont leurs propres captures)
        Expr::Nameless { .. } | Expr::Literal(..) | Expr::StaticConst { .. } => {}
    }
}
