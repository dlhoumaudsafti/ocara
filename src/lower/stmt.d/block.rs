/// Lowering de blocs

use crate::parsing::ast::Block;
use crate::lower::builder::LowerBuilder;
use super::statements::lower_stmt;
use super::ownership::{drop_consumed_used_in, emit_scope_drops};

pub fn lower_block(builder: &mut LowerBuilder, block: &Block) {
    // Nouvelle frame pour ce bloc — alimentée par `register_owned_local`,
    // consultée par `emit_early_exit_drops` sur un `return`/`break`/
    // `continue` anticipé (voir la doc de `block_scope_stack`).
    builder.block_scope_stack.push(Vec::new());

    for stmt in &block.stmts {
        if builder.is_terminated() { break; }
        lower_stmt(builder, stmt);
        // `consumed` : détruite juste après son unique usage permis, s'il
        // se trouve dans ce statement (voir crate::lower::stmt::ownership).
        if !builder.is_terminated() {
            drop_consumed_used_in(builder, stmt);
        }
    }
    // `scoped`, et `consumed` jamais utilisée : détruites en fin de bloc —
    // seulement si le bloc se termine normalement (pas de return/raise/
    // break/continue déjà émis, voir la doc de crate::lower::stmt::ownership).
    // Un `return`/`break`/`continue` a déjà émis ses propres destructions
    // via `emit_early_exit_drops` avant son terminateur — pas la peine de
    // les refaire ici, `is_terminated()` protège justement contre ça.
    if !builder.is_terminated() {
        emit_scope_drops(builder, block);
    }

    builder.block_scope_stack.pop();
}
