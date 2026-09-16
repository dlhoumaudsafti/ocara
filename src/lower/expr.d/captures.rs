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
    walk_block_caps(body, param_names, locals, &mut caps, &mut seen, true);
    caps
}

/// Comme `collect_captures`, mais pour un bloc qui N'EST PAS le corps d'une
/// fermeture (typiquement le corps d'une boucle `while`/`for`) : ignore les
/// références DIRECTES de `body` lui-même (une variable de boucle ordinaire
/// comme `i` dans `while i smaller N {...}` ne doit JAMAIS être promue au tas
/// juste parce qu'elle y est référencée) — ne récolte que ce dont une
/// fermeture (`Expr::Nameless`) À L'INTÉRIEUR de `body`, à n'importe quelle
/// profondeur, a réellement besoin.
///
/// Utilisée pour pré-promouvoir, AVANT le corps d'une boucle, toute variable
/// qu'une fermeture créée dans ce corps va capturer — sans ce pré-scan, la
/// promotion (déclenchée normalement au premier `Expr::Nameless` rencontré
/// lors du lowering) atterrit À L'INTÉRIEUR du corps de boucle, donc
/// ré-exécutée à CHAQUE itération au runtime : chaque passage alloue une
/// NOUVELLE cellule heap, réinitialisée depuis le slot stack d'origine
/// (jamais mis à jour après la toute première promotion), perdant l'état
/// accumulé des itérations précédentes — confirmé par reproduction
/// (`ocara --dump`), voir
/// docs/roadmap.d/langage-closure-promotion-in-loop.md.
pub fn names_captured_by_nested_closures(
    body:   &Block,
    locals: &HashMap<String, (Value, IrType, bool)>,
) -> Vec<(String, IrType)> {
    let mut caps: Vec<(String, IrType)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    walk_block_caps(body, &HashSet::new(), locals, &mut caps, &mut seen, false);
    caps
}

fn walk_block_caps(b: &Block, p: &HashSet<String>, l: &HashMap<String, (Value, IrType, bool)>, caps: &mut Vec<(String, IrType)>, seen: &mut HashSet<String>, count_direct_refs: bool) {
    for stmt in &b.stmts { walk_stmt_caps(stmt, p, l, caps, seen, count_direct_refs); }
}

fn walk_stmt_caps(stmt: &Stmt, p: &HashSet<String>, l: &HashMap<String, (Value, IrType, bool)>, caps: &mut Vec<(String, IrType)>, seen: &mut HashSet<String>, count_direct_refs: bool) {
    match stmt {
        Stmt::Var   { value, .. }     => walk_expr_caps(value, p, l, caps, seen, count_direct_refs),
        Stmt::Const { value, .. }     => walk_expr_caps(value, p, l, caps, seen, count_direct_refs),
        Stmt::Expr(e)                 => walk_expr_caps(e, p, l, caps, seen, count_direct_refs),
        Stmt::Assign { target, value, .. } => {
            walk_expr_caps(target, p, l, caps, seen, count_direct_refs);
            walk_expr_caps(value,  p, l, caps, seen, count_direct_refs);
        }
        Stmt::Return { value: Some(e), .. } | Stmt::Result { value: Some(e), .. } => walk_expr_caps(e, p, l, caps, seen, count_direct_refs),
        Stmt::Return { .. } | Stmt::Result { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {}
        Stmt::If { condition, then_block, elseif, else_block, .. } => {
            walk_expr_caps(condition, p, l, caps, seen, count_direct_refs);
            walk_block_caps(then_block, p, l, caps, seen, count_direct_refs);
            for (c, blk) in elseif { walk_expr_caps(c, p, l, caps, seen, count_direct_refs); walk_block_caps(blk, p, l, caps, seen, count_direct_refs); }
            if let Some(blk) = else_block { walk_block_caps(blk, p, l, caps, seen, count_direct_refs); }
        }
        Stmt::While { condition, body, .. } => {
            walk_expr_caps(condition, p, l, caps, seen, count_direct_refs);
            walk_block_caps(body, p, l, caps, seen, count_direct_refs);
        }
        Stmt::ForIn { iter, body, .. } => {
            walk_expr_caps(iter, p, l, caps, seen, count_direct_refs);
            walk_block_caps(body, p, l, caps, seen, count_direct_refs);
        }
        Stmt::ForMap { iter, body, .. } => {
            walk_expr_caps(iter, p, l, caps, seen, count_direct_refs);
            walk_block_caps(body, p, l, caps, seen, count_direct_refs);
        }
        Stmt::Switch { subject, cases, default, .. } => {
            walk_expr_caps(subject, p, l, caps, seen, count_direct_refs);
            for c in cases { walk_block_caps(&c.body, p, l, caps, seen, count_direct_refs); }
            if let Some(blk) = default { walk_block_caps(blk, p, l, caps, seen, count_direct_refs); }
        }
        Stmt::Try { body, handlers, .. } => {
            walk_block_caps(body, p, l, caps, seen, count_direct_refs);
            for h in handlers { walk_block_caps(&h.body, p, l, caps, seen, count_direct_refs); }
        }
        Stmt::Raise { value, .. } => walk_expr_caps(value, p, l, caps, seen, count_direct_refs),
        Stmt::Emit { value, .. } => walk_expr_caps(value, p, l, caps, seen, count_direct_refs),
    }
}

fn walk_expr_caps(expr: &Expr, p: &HashSet<String>, l: &HashMap<String, (Value, IrType, bool)>, caps: &mut Vec<(String, IrType)>, seen: &mut HashSet<String>, count_direct_refs: bool) {
    match expr {
        Expr::Ident(name, _) => {
            if count_direct_refs && !p.contains(name.as_str()) && !seen.contains(name.as_str()) {
                if let Some((_, ty, _)) = l.get(name.as_str()) {
                    caps.push((name.clone(), ty.clone()));
                    seen.insert(name.clone());
                }
            }
        }
        Expr::SelfExpr(_) | Expr::ParentExpr(_) => {
            let key = "self";
            if count_direct_refs && !seen.contains(key) {
                // self/parent est un paramètre de fonction, pas un local
                // On doit l'inclure comme capture avec son type
                // Dans le contexte d'une méthode, self est toujours de type I64 (pointeur)
                caps.push((key.to_string(), IrType::I64));
                seen.insert(key.to_string());
            }
        }
        Expr::Binary { left, right, .. } => { walk_expr_caps(left, p, l, caps, seen, count_direct_refs); walk_expr_caps(right, p, l, caps, seen, count_direct_refs); }
        Expr::Unary  { operand, .. }     => walk_expr_caps(operand, p, l, caps, seen, count_direct_refs),
        Expr::Field  { object, .. }      => walk_expr_caps(object, p, l, caps, seen, count_direct_refs),
        Expr::Call   { callee, args, .. } => { walk_expr_caps(callee, p, l, caps, seen, count_direct_refs); for a in args { walk_expr_caps(a, p, l, caps, seen, count_direct_refs); } }
        Expr::StaticCall { args, .. }    => { for a in args { walk_expr_caps(a, p, l, caps, seen, count_direct_refs); } }
        Expr::New    { args, .. }        => { for a in args { walk_expr_caps(a, p, l, caps, seen, count_direct_refs); } }
        Expr::Index  { object, index, ..} => { walk_expr_caps(object, p, l, caps, seen, count_direct_refs); walk_expr_caps(index, p, l, caps, seen, count_direct_refs); }
        Expr::Range  { start, end, .. }  => { walk_expr_caps(start, p, l, caps, seen, count_direct_refs); walk_expr_caps(end, p, l, caps, seen, count_direct_refs); }
        Expr::Array  { elements, .. }    => { for e in elements { walk_expr_caps(e, p, l, caps, seen, count_direct_refs); } }
        Expr::Map    { entries, .. }     => { for (k, v) in entries { walk_expr_caps(k, p, l, caps, seen, count_direct_refs); walk_expr_caps(v, p, l, caps, seen, count_direct_refs); } }
        Expr::Template { parts, .. }     => { for part in parts { if let TemplatePartExpr::Expr(e) = part { walk_expr_caps(e, p, l, caps, seen, count_direct_refs); } } }
        Expr::Match  { subject, arms, ..} => { walk_expr_caps(subject, p, l, caps, seen, count_direct_refs); for arm in arms { walk_expr_caps(&arm.body, p, l, caps, seen, count_direct_refs); } }
        Expr::IsCheck { expr, .. }       => walk_expr_caps(expr, p, l, caps, seen, count_direct_refs),
        Expr::Resolve { expr, .. }        => walk_expr_caps(expr, p, l, caps, seen, count_direct_refs),
        Expr::IncDec { target, .. }      => walk_expr_caps(target, p, l, caps, seen, count_direct_refs),
        // Ne pas descendre dans le CORPS d'une nameless imbriquée pour lui
        // faire porter directement ses propres références (elle a ses
        // propres captures, résolues indépendamment lors de son propre
        // lowering) — mais calculer RÉCURSIVEMENT ce dont elle a besoin et
        // remonter dans NOS PROPRES captures tout nom qu'elle référence et
        // qui nous est visible (`l`), pour qu'on le retransmette à notre
        // tour. Sans ceci, une fermeture qui ne référence PAS elle-même une
        // variable — mais dont une fermeture qu'elle contient en a besoin —
        // ne la capture jamais : au moment de lower cette fermeture interne,
        // la variable est absente de `locals`/`captured_vars` de la
        // fermeture englobante (qui ne l'a jamais elle-même capturée), donc
        // introuvable à N'IMPORTE QUELLE profondeur au-delà du premier
        // niveau — confirmé par reproduction (`ocara --dump` : l'env de la
        // fermeture interne était un `ConstInt 0`/NULL littéral, pas une
        // structure de capture), voir
        // docs/roadmap.d/langage-nested-closure-recapture.md. Récursif par
        // construction (si la fermeture imbriquée contient elle-même une
        // fermeture encore plus interne, son propre appel à
        // `collect_captures` applique la même règle) — sûr : `body` d'une
        // `Expr::Nameless` est toujours strictement plus petit que l'arbre
        // englobant, aucun risque de cycle.
        //
        // TOUJOURS actif, indépendamment de `count_direct_refs` : que `body`
        // (le bloc englobant CETTE fermeture-ci) soit lui-même le corps d'une
        // fermeture (`collect_captures`) ou un simple corps de boucle
        // (`names_captured_by_nested_closures`), une fermeture RÉELLEMENT
        // créée ici a TOUJOURS besoin de ses propres captures — seule la
        // question "faut-il aussi compter les références DIRECTES du bloc
        // englobant lui-même" varie selon l'appelant.
        Expr::Nameless { params, body, .. } => {
            let nested_params: HashSet<String> = params.iter().map(|param| param.name.clone()).collect();
            let nested_caps = collect_captures(body, &nested_params, l);
            for (name, ty) in nested_caps {
                if !p.contains(name.as_str()) && !seen.contains(name.as_str()) {
                    seen.insert(name.clone());
                    caps.push((name, ty));
                }
            }
        }
        Expr::Literal(..) | Expr::StaticConst { .. } => {}
    }
}
