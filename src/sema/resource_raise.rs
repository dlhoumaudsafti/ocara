/// Détecte qu'une `scoped`/`consumed` ressource (Mutex/SQLite/MySQL/MariaDB/
/// HTTPRequest/HTTPResponse/Thread) reste "ouverte" (pas encore finalisée
/// manuellement) au moment d'un `raise` qui n'est PAS localement rattrapé par
/// un `try` dans le MÊME bloc — voir
/// docs/roadmap.d/exceptions-setjmp-longjmp-dette.md : le mécanisme
/// d'exceptions d'Ocara (`setjmp`/`longjmp`, voir
/// `src/lower/stmt.d/statements.d/exceptions.rs`) saute par-dessus tout code
/// de nettoyage intermédiaire, y compris la finalisation automatique de fin
/// de bloc d'une `scoped`/`consumed` (voir `docs/EBNF.md` §9.2) — fuite
/// (SQLite/MySQL/MariaDB/HTTPRequest/HTTPResponse) ou deadlock permanent
/// (Mutex jamais déverrouillé, Thread jamais rejoint/détaché) pour tout code
/// qui dépendrait ensuite de cette ressource.
///
/// Volontairement CONSERVATEUR (même philosophie que E28, voir
/// `crate::sema::scope::UnclosedResourceVar`, et le mode non strict de
/// `crate::sema::escape`) — quitte à manquer un vrai risque plutôt que
/// signaler du code parfaitement sûr :
///   - un `raise` À L'INTÉRIEUR d'un `try` imbriqué est considéré rattrapé
///     localement pour tout ce qui est déclaré AVANT/EN DEHORS de ce `try`
///     (sans vérifier que ses `on` couvrent réellement la classe levée —
///     complexité d'appariement de types délibérément hors périmètre ici) ;
///   - un `raise` dans un `on` HANDLER, en revanche, compte comme
///     atteignant le bloc englobant (rien ne le protège plus, son propre
///     `try` a déjà servi) ;
///   - une finalisation (`.join()`/`.detach()`/`.destroy()`/`.close()`)
///     appelée en LIGNE DROITE (pas dans un `if`/`while` imbriqué) avant le
///     `raise`, dans le MÊME bloc, supprime l'avertissement pour cette
///     ressource précise ;
///   - un `raise` atteignable via un `if`/`while`/`for`/`switch` imbriqué
///     (jamais protégé par son propre `try`) compte comme atteignant le
///     bloc englobant, puisque rien n'arrête sa propagation à cette
///     frontière.
/// Aucune analyse interprocédurale : seul un `raise` TEXTUEL compte, pas un
/// appel vers une fonction qui pourrait elle-même lever une exception —
/// cohérent avec le mode non strict de `crate::sema::escape` (E26).
use std::collections::HashSet;
use crate::parsing::ast::*;
use crate::parsing::token::Span;
use crate::sema::error::SemaWarning;
use crate::sema::scope::{ownership_class_of, OwnershipClass};

pub fn check_program(program: &Program, resource_classes: &HashSet<String>) -> Vec<SemaWarning> {
    let mut warnings = Vec::new();
    for func in &program.functions {
        scan_block(&func.body, resource_classes, &mut warnings);
    }
    for class in &program.classes {
        scan_members(&class.members, resource_classes, &mut warnings);
    }
    for module in &program.modules {
        scan_members(&module.members, resource_classes, &mut warnings);
    }
    warnings
}

fn scan_members(members: &[ClassMember], resource_classes: &HashSet<String>, warnings: &mut Vec<SemaWarning>) {
    for member in members {
        match member {
            ClassMember::Constructor { body, .. } => scan_block(body, resource_classes, warnings),
            ClassMember::Method { decl, .. } => scan_block(&decl.body, resource_classes, warnings),
            ClassMember::Field { .. } | ClassMember::Const { .. } => {}
        }
    }
}

/// Nom de classe (`Type::Named`) pour une ressource "possédable" au sens de
/// ce diagnostic — `OwnershipClass::Resource` (ressource nue OU classe
/// utilisateur en contenant une, voir `resource_classes`) ou `Thread` (même
/// contrainte de finalisation manuelle, méthodes différentes — voir
/// `crate::sema::scope::ownership_class_of`).
fn resource_class_name<'a>(ty: &'a Type, resource_classes: &HashSet<String>) -> Option<&'a str> {
    if let Type::Named(n) = ty {
        if matches!(ownership_class_of(ty, resource_classes), OwnershipClass::Resource | OwnershipClass::Thread) {
            return Some(n.as_str());
        }
    }
    None
}

/// Vrai si `field` finalise manuellement une ressource/Thread — mêmes noms
/// de méthode que `crate::sema::scope::ScopeStack::mark_resource_finalized`.
fn is_finalizing_method(field: &str) -> bool {
    matches!(field, "destroy" | "close" | "join" | "detach")
}

type OpenResource = (String, String, Span);

fn scan_block(block: &Block, resource_classes: &HashSet<String>, warnings: &mut Vec<SemaWarning>) {
    // Ressources `scoped`/`consumed` déclarées DIRECTEMENT dans CE bloc, pas
    // encore finalisées en ligne droite ni déjà signalées.
    let mut open: Vec<OpenResource> = Vec::new();
    let mut warned: HashSet<String> = HashSet::new();

    for stmt in &block.stmts {
        match stmt {
            Stmt::Var { name, ty, kind, span, .. } if matches!(kind, VarKind::Scoped | VarKind::Consumed) => {
                if let Some(class_name) = resource_class_name(ty, resource_classes) {
                    open.push((name.clone(), class_name.to_string(), span.clone()));
                }
            }
            Stmt::Expr(Expr::Call { callee, .. }) => {
                if let Expr::Field { object, field, .. } = callee.as_ref() {
                    if let Expr::Ident(recv, _) = object.as_ref() {
                        if is_finalizing_method(field) {
                            open.retain(|(n, _, _)| n != recv.as_str());
                        }
                    }
                }
            }
            Stmt::Raise { .. } => warn_open(&open, warnings, &mut warned),
            Stmt::If { then_block, elseif, else_block, .. } => {
                check_nested(then_block, &open, warnings, &mut warned);
                for (_, blk) in elseif { check_nested(blk, &open, warnings, &mut warned); }
                if let Some(blk) = else_block { check_nested(blk, &open, warnings, &mut warned); }
                scan_block(then_block, resource_classes, warnings);
                for (_, blk) in elseif { scan_block(blk, resource_classes, warnings); }
                if let Some(blk) = else_block { scan_block(blk, resource_classes, warnings); }
            }
            Stmt::While { body, .. } | Stmt::ForIn { body, .. } | Stmt::ForMap { body, .. } => {
                check_nested(body, &open, warnings, &mut warned);
                scan_block(body, resource_classes, warnings);
            }
            Stmt::Switch { cases, default, .. } => {
                for c in cases { check_nested(&c.body, &open, warnings, &mut warned); scan_block(&c.body, resource_classes, warnings); }
                if let Some(d) = default { check_nested(d, &open, warnings, &mut warned); scan_block(d, resource_classes, warnings); }
            }
            Stmt::Try { body, handlers, .. } => {
                // `body` : un `raise` ici est assumé rattrapé localement pour
                // les ressources du bloc ENGLOBANT (voir la doc de module) —
                // ne pas vérifier `open` contre son contenu.
                scan_block(body, resource_classes, warnings);
                // Les HANDLERS, en revanche : plus aucun `try` ne les protège
                // — un `raise` dedans atteint le bloc englobant normalement.
                for h in handlers {
                    check_nested(&h.body, &open, warnings, &mut warned);
                    scan_block(&h.body, resource_classes, warnings);
                }
            }
            _ => {}
        }
    }
}

fn warn_open(open: &[OpenResource], warnings: &mut Vec<SemaWarning>, warned: &mut HashSet<String>) {
    for (name, class_name, decl_span) in open {
        if warned.insert(name.clone()) {
            warnings.push(SemaWarning::ScopedResourceRaiseLeak {
                name: name.clone(),
                class_name: class_name.clone(),
                span: decl_span.clone(),
            });
        }
    }
}

/// Vérifie si `block` (un `if`/`while`/`for`/`switch`/handler `on` imbriqué,
/// PAS le corps protégé d'un `try`) contient un `raise` atteignable sans
/// passer par un `try` qui lui est propre — si oui, chaque ressource encore
/// `open` (du bloc PARENT) est signalée une fois.
fn check_nested(block: &Block, open: &[OpenResource], warnings: &mut Vec<SemaWarning>, warned: &mut HashSet<String>) {
    if !open.is_empty() && block_reaches_uncaught_raise(block) {
        warn_open(open, warnings, warned);
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
        // Un `try` imbriqué protège tout ce qui est DANS son propre corps —
        // jamais considéré comme "atteignant" le bloc au-delà de lui-même.
        // Ses handlers, eux, comptent (plus aucun `try` ne les couvre).
        Stmt::Try { handlers, .. } => handlers.iter().any(|h| block_reaches_uncaught_raise(&h.body)),
        _ => false,
    }
}
