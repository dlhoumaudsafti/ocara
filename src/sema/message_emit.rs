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
}

pub fn analyze_emit(body: &Block) -> EmitAnalysis {
    let mut has_emit = false;
    let mut emit_in_loop = false;
    walk_block(body, false, &mut has_emit, &mut emit_in_loop);
    EmitAnalysis { has_emit, emit_in_loop }
}

fn walk_block(block: &Block, in_loop: bool, has_emit: &mut bool, emit_in_loop: &mut bool) {
    walk_stmts(&block.stmts, in_loop, has_emit, emit_in_loop);
}

fn walk_stmts(stmts: &[Stmt], in_loop: bool, has_emit: &mut bool, emit_in_loop: &mut bool) {
    for stmt in stmts {
        match stmt {
            Stmt::Emit { .. } => {
                *has_emit = true;
                if in_loop {
                    *emit_in_loop = true;
                }
            }
            Stmt::If { then_block, elseif, else_block, .. } => {
                walk_block(then_block, in_loop, has_emit, emit_in_loop);
                for (_, blk) in elseif {
                    walk_block(blk, in_loop, has_emit, emit_in_loop);
                }
                if let Some(blk) = else_block {
                    walk_block(blk, in_loop, has_emit, emit_in_loop);
                }
            }
            Stmt::Switch { cases, default, .. } => {
                for case in cases {
                    walk_block(&case.body, in_loop, has_emit, emit_in_loop);
                }
                if let Some(blk) = default {
                    walk_block(blk, in_loop, has_emit, emit_in_loop);
                }
            }
            Stmt::While { body, .. } => {
                walk_block(body, true, has_emit, emit_in_loop);
            }
            Stmt::ForIn { body, .. } => {
                walk_block(body, true, has_emit, emit_in_loop);
            }
            Stmt::ForMap { body, .. } => {
                walk_block(body, true, has_emit, emit_in_loop);
            }
            Stmt::Try { body, handlers, .. } => {
                walk_block(body, in_loop, has_emit, emit_in_loop);
                for handler in handlers {
                    walk_block(&handler.body, in_loop, has_emit, emit_in_loop);
                }
            }
            // Ni bloc imbriqué, ni `emit` possible directement.
            Stmt::Var { .. } | Stmt::Const { .. } | Stmt::Expr(_) | Stmt::Return { .. }
            | Stmt::Result { .. } | Stmt::Break { .. } | Stmt::Continue { .. }
            | Stmt::Raise { .. } | Stmt::Assign { .. } => {}
        }
    }
}
