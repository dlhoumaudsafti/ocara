/// Lowering de blocs

use crate::parsing::ast::Block;
use crate::lower::builder::LowerBuilder;
use super::statements::lower_stmt;
use super::ownership::{drop_consumed_used_in, emit_scope_drops};

pub fn lower_block(builder: &mut LowerBuilder, block: &Block) {
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
    if !builder.is_terminated() {
        emit_scope_drops(builder, block);
    }
}
