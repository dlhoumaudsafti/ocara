/// Analyse statique de `emit` (générateurs, type `message<T>`) — voir
/// docs/roadmap.d/langage-emit-iterable.md.
///
/// Un seul walker, partagé entre l'enregistrement des symboles
/// (`crate::sema::symbols::registers`, pour peupler `FuncSig`) et le
/// typecheck (`crate::sema::typecheck::check_func`, qui a directement le
/// `Block` sous la main et n'a pas besoin de repasser par la table des
/// symboles).
use crate::parsing::ast::{Block, Stmt};

/// Résultat de l'analyse d'un corps de fonction/méthode vis-à-vis de `emit`.
pub struct EmitAnalysis {
    /// Au moins un `emit` atteignable (n'importe où dans le corps).
    pub has_emit: bool,
    /// Au moins un `emit` atteignable À L'INTÉRIEUR d'une boucle (`while`/
    /// `for`) — dans ce cas, on ne peut PLUS prouver statiquement qu'au plus
    /// un seul `emit` est jamais atteint : la consommation directe en valeur
    /// scalaire (`var x:T = truc()`) est interdite, seuls `for`/
    /// `Array::fromMessage` restent valables (voir la fiche roadmap, section
    /// "Conception actée").
    pub emit_in_loop: bool,
    /// Au moins un `emit` atteignable À L'INTÉRIEUR d'un `try` (Cas A de la
    /// fiche roadmap, §4) — le lowering actuel (machine à états, voir
    /// `crate::lower::builder::message_gen`) ne rejoue pas encore les
    /// `setjmp`/`TryFrame` nécessaires à chaque reprise (Étape 5, pas encore
    /// construite) : lowered tel quel, ce `emit` ferait un `Return` depuis
    /// `__try_body_N` (une fonction SÉPARÉE, voir
    /// `crate::lower::stmt::exceptions::lower_try`) au lieu de suspendre le
    /// générateur — miscompilation silencieuse, pas juste une limite
    /// acceptée. Rejeté à la compilation tant que l'Étape 5 n'existe pas.
    pub emit_in_try: bool,
}

pub fn analyze_emit(body: &Block) -> EmitAnalysis {
    let mut has_emit = false;
    let mut emit_in_loop = false;
    let mut emit_in_try = false;
    walk_block(body, false, false, &mut has_emit, &mut emit_in_loop, &mut emit_in_try);
    EmitAnalysis { has_emit, emit_in_loop, emit_in_try }
}

fn walk_block(
    block: &Block,
    in_loop: bool,
    in_try: bool,
    has_emit: &mut bool,
    emit_in_loop: &mut bool,
    emit_in_try: &mut bool,
) {
    walk_stmts(&block.stmts, in_loop, in_try, has_emit, emit_in_loop, emit_in_try);
}

fn walk_stmts(
    stmts: &[Stmt],
    in_loop: bool,
    in_try: bool,
    has_emit: &mut bool,
    emit_in_loop: &mut bool,
    emit_in_try: &mut bool,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Emit { .. } => {
                *has_emit = true;
                if in_loop {
                    *emit_in_loop = true;
                }
                if in_try {
                    *emit_in_try = true;
                }
            }
            Stmt::If { then_block, elseif, else_block, .. } => {
                walk_block(then_block, in_loop, in_try, has_emit, emit_in_loop, emit_in_try);
                for (_, blk) in elseif {
                    walk_block(blk, in_loop, in_try, has_emit, emit_in_loop, emit_in_try);
                }
                if let Some(blk) = else_block {
                    walk_block(blk, in_loop, in_try, has_emit, emit_in_loop, emit_in_try);
                }
            }
            Stmt::Switch { cases, default, .. } => {
                for case in cases {
                    walk_block(&case.body, in_loop, in_try, has_emit, emit_in_loop, emit_in_try);
                }
                if let Some(blk) = default {
                    walk_block(blk, in_loop, in_try, has_emit, emit_in_loop, emit_in_try);
                }
            }
            Stmt::While { body, .. } => {
                walk_block(body, true, in_try, has_emit, emit_in_loop, emit_in_try);
            }
            Stmt::ForIn { body, .. } => {
                walk_block(body, true, in_try, has_emit, emit_in_loop, emit_in_try);
            }
            Stmt::ForMap { body, .. } => {
                walk_block(body, true, in_try, has_emit, emit_in_loop, emit_in_try);
            }
            Stmt::Try { body, handlers, .. } => {
                walk_block(body, in_loop, true, has_emit, emit_in_loop, emit_in_try);
                for handler in handlers {
                    // Un `emit` dans un HANDLER (`on e { emit ... }`) n'est
                    // pas le Cas A (qui concerne le CORPS du try, voir §4) —
                    // un handler n'est de toute façon lowered qu'en cas de
                    // `raise`, jamais un point de suspension normal.
                    walk_block(&handler.body, in_loop, in_try, has_emit, emit_in_loop, emit_in_try);
                }
            }
            // Ni bloc imbriqué, ni `emit` possible directement.
            Stmt::Var { .. } | Stmt::Const { .. } | Stmt::Expr(_) | Stmt::Return { .. }
            | Stmt::Result { .. } | Stmt::Break { .. } | Stmt::Continue { .. }
            | Stmt::Raise { .. } | Stmt::Assign { .. } => {}
        }
    }
}
