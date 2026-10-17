/// W04 : un `raise` non rattrapé localement saute une finalisation que le
/// déroulement des exceptions ne fait PAS à la place du développeur.
///
/// Le déroulement (`__rc_resource_push`, `src/lower/stmt.d/ownership.rs`)
/// ferme les ressources natives `scoped`/`consumed` (Mutex/SQLite/MySQL/
/// MariaDB/HTTPRequest/HTTPResponse) et rend les références des objets
/// (une classe utilisateur porteuse de ressource est fermée à sa
/// destruction) — rien à signaler pour elles. Restent deux cas :
///   - un `Thread` (`scoped`/`consumed`/`var`) : un `raise` saute son
///     `.join()`/`.detach()`, le thread est abandonné (jamais attendu) ;
///   - une ressource native déclarée `var` : rien ne la ferme à la place du
///     développeur, un `raise` saute son `.close()`/`.destroy()` manuel —
///     fuite, ou `Mutex` laissé verrouillé.
///
/// Volontairement CONSERVATEUR — quitte à manquer un vrai risque plutôt que
/// signaler du code sûr :
///   - un `raise` À L'INTÉRIEUR d'un `try` imbriqué est considéré rattrapé
///     localement (sans vérifier que ses `on` couvrent la classe levée) ;
///   - un `raise` dans un `on` HANDLER compte comme atteignant le bloc
///     englobant (plus aucun `try` ne le protège) ;
///   - une finalisation (`.join()`/`.detach()`/`.destroy()`/`.close()`) en
///     LIGNE DROITE avant le `raise`, dans le MÊME bloc, supprime
///     l'avertissement pour cette variable ;
///   - une variable `var` n'est signalée que si sa finalisation manuelle
///     apparaît APRÈS le `raise` dans le même bloc (le `raise` la saute
///     réellement) — une ressource `var` partagée et jamais fermée ici
///     n'est pas concernée ;
///   - un `raise` atteignable via un `if`/`while`/`for`/`switch` imbriqué
///     (sans son propre `try`) compte comme atteignant le bloc englobant.
/// Aucune analyse interprocédurale : seul un `raise` TEXTUEL compte.
use std::collections::HashSet;
use crate::parsing::ast::*;
use crate::parsing::token::Span;
use crate::sema::error::SemaWarning;
use crate::sema::scope::{ownership_class, OwnershipClass};

pub fn check_program(program: &Program) -> Vec<SemaWarning> {
    let mut warnings = Vec::new();
    for func in &program.functions {
        scan_block(&func.body, &mut warnings);
    }
    for class in &program.classes {
        scan_members(&class.members, &mut warnings);
    }
    for module in &program.modules {
        scan_members(&module.members, &mut warnings);
    }
    warnings
}

fn scan_members(members: &[ClassMember], warnings: &mut Vec<SemaWarning>) {
    for member in members {
        match member {
            ClassMember::Constructor { body, .. } => scan_block(body, warnings),
            ClassMember::Method { decl, .. } => scan_block(&decl.body, warnings),
            ClassMember::Field { .. } | ClassMember::Const { .. } => {}
        }
    }
}

/// Variable suivie, pas encore finalisée en ligne droite.
struct Tracked {
    name:       String,
    class_name: String,
    span:       Span,
    is_thread:  bool,
    /// `var` : signalée seulement si sa finalisation suit le `raise`.
    manual:     bool,
}

/// `Some(Tracked)` si la déclaration relève de W04 (voir la doc du module).
fn tracked(name: &str, ty: &Type, kind: &VarKind, span: &Span) -> Option<Tracked> {
    let Type::Named(class_name) = ty else { return None };
    let is_thread = match ownership_class(ty) {
        OwnershipClass::Thread => true,
        OwnershipClass::Resource if *kind == VarKind::Var => false,
        _ => return None,
    };
    Some(Tracked {
        name: name.to_string(),
        class_name: class_name.clone(),
        span: span.clone(),
        is_thread,
        manual: *kind == VarKind::Var,
    })
}

/// Vrai si `field` finalise manuellement une ressource/Thread — mêmes noms
/// de méthode que `crate::sema::scope::ScopeStack::mark_resource_finalized`.
fn is_finalizing_method(field: &str) -> bool {
    matches!(field, "destroy" | "close" | "join" | "detach")
}

/// `recv` si `stmt` est un appel `recv.<finalisation>()` en instruction.
fn finalized_name(stmt: &Stmt) -> Option<&str> {
    let Stmt::Expr(Expr::Call { callee, .. }) = stmt else { return None };
    let Expr::Field { object, field, .. } = callee.as_ref() else { return None };
    let Expr::Ident(recv, _) = object.as_ref() else { return None };
    is_finalizing_method(field).then_some(recv.as_str())
}

fn scan_block(block: &Block, warnings: &mut Vec<SemaWarning>) {
    let mut open: Vec<Tracked> = Vec::new();
    let mut warned: HashSet<String> = HashSet::new();

    for (i, stmt) in block.stmts.iter().enumerate() {
        let rest = &block.stmts[i + 1..];
        match stmt {
            Stmt::Var { name, ty, kind, span, .. } => {
                if let Some(t) = tracked(name, ty, kind, span) {
                    open.push(t);
                }
            }
            Stmt::Expr(_) => {
                if let Some(recv) = finalized_name(stmt) {
                    open.retain(|t| t.name != recv);
                }
            }
            Stmt::Raise { .. } => warn_open(&open, rest, warnings, &mut warned),
            Stmt::If { then_block, elseif, else_block, .. } => {
                let mut blocks = vec![then_block];
                blocks.extend(elseif.iter().map(|(_, b)| b));
                blocks.extend(else_block.iter());
                for blk in blocks {
                    check_nested(blk, &open, rest, warnings, &mut warned);
                    scan_block(blk, warnings);
                }
            }
            Stmt::While { body, .. } | Stmt::ForIn { body, .. } | Stmt::ForMap { body, .. } => {
                check_nested(body, &open, rest, warnings, &mut warned);
                scan_block(body, warnings);
            }
            Stmt::Switch { cases, default, .. } => {
                for c in cases {
                    check_nested(&c.body, &open, rest, warnings, &mut warned);
                    scan_block(&c.body, warnings);
                }
                if let Some(d) = default {
                    check_nested(d, &open, rest, warnings, &mut warned);
                    scan_block(d, warnings);
                }
            }
            Stmt::Try { body, handlers, .. } => {
                // `body` : un `raise` ici est assumé rattrapé localement.
                scan_block(body, warnings);
                // Les handlers, eux, ne sont plus protégés par aucun `try`.
                for h in handlers {
                    check_nested(&h.body, &open, rest, warnings, &mut warned);
                    scan_block(&h.body, warnings);
                }
            }
            _ => {}
        }
    }
}

/// Signale (une fois) chaque variable de `open` dont un `raise` saute la
/// finalisation ; `rest` = instructions du bloc qui suivent ce `raise`.
fn warn_open(open: &[Tracked], rest: &[Stmt], warnings: &mut Vec<SemaWarning>, warned: &mut HashSet<String>) {
    for t in open {
        if t.manual && !finalized_later(rest, &t.name) {
            continue;
        }
        if warned.insert(t.name.clone()) {
            warnings.push(SemaWarning::RaiseSkipsFinalization {
                name: t.name.clone(),
                class_name: t.class_name.clone(),
                is_thread: t.is_thread,
                span: t.span.clone(),
            });
        }
    }
}

/// Vrai si `name` est finalisée quelque part dans `stmts` (à toute profondeur).
fn finalized_later(stmts: &[Stmt], name: &str) -> bool {
    stmts.iter().any(|s| {
        if finalized_name(s) == Some(name) {
            return true;
        }
        match s {
            Stmt::If { then_block, elseif, else_block, .. } => {
                finalized_later(&then_block.stmts, name)
                    || elseif.iter().any(|(_, b)| finalized_later(&b.stmts, name))
                    || else_block.as_ref().is_some_and(|b| finalized_later(&b.stmts, name))
            }
            Stmt::While { body, .. } | Stmt::ForIn { body, .. } | Stmt::ForMap { body, .. } => finalized_later(&body.stmts, name),
            Stmt::Switch { cases, default, .. } => {
                cases.iter().any(|c| finalized_later(&c.body.stmts, name))
                    || default.as_ref().is_some_and(|b| finalized_later(&b.stmts, name))
            }
            Stmt::Try { body, handlers, .. } => {
                finalized_later(&body.stmts, name) || handlers.iter().any(|h| finalized_later(&h.body.stmts, name))
            }
            _ => false,
        }
    })
}

/// Si `block` (un `if`/`while`/`for`/`switch`/handler `on` imbriqué, PAS le
/// corps protégé d'un `try`) contient un `raise` atteignable sans `try`
/// propre, chaque variable encore `open` du bloc PARENT est examinée. Une
/// finalisation placée dans `block` après ce `raise` ne compte pas : seul
/// `rest` (la suite du bloc parent) est consulté, conservateur.
fn check_nested(block: &Block, open: &[Tracked], rest: &[Stmt], warnings: &mut Vec<SemaWarning>, warned: &mut HashSet<String>) {
    if !open.is_empty() && block_reaches_uncaught_raise(block) {
        warn_open(open, rest, warnings, warned);
    }
}

fn block_reaches_uncaught_raise(block: &Block) -> bool {
    block.stmts.iter().any(stmt_reaches_uncaught_raise)
}

fn stmt_reaches_uncaught_raise(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Raise { .. } => true,
        Stmt::If { then_block, elseif, else_block, .. } => {
            block_reaches_uncaught_raise(then_block)
                || elseif.iter().any(|(_, b)| block_reaches_uncaught_raise(b))
                || else_block.as_ref().is_some_and(|b| block_reaches_uncaught_raise(b))
        }
        Stmt::While { body, .. } | Stmt::ForIn { body, .. } | Stmt::ForMap { body, .. } => block_reaches_uncaught_raise(body),
        Stmt::Switch { cases, default, .. } => {
            cases.iter().any(|c| block_reaches_uncaught_raise(&c.body))
                || default.as_ref().is_some_and(|b| block_reaches_uncaught_raise(b))
        }
        // Un `try` imbriqué protège son propre corps ; ses handlers, non.
        Stmt::Try { handlers, .. } => handlers.iter().any(|h| block_reaches_uncaught_raise(&h.body)),
        _ => false,
    }
}
