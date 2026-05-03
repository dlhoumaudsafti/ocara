/// Lowering de blocs

use crate::parsing::ast::Block;
use crate::lower::builder::LowerBuilder;
use super::statements::lower_stmt;

pub fn lower_block(builder: &mut LowerBuilder, block: &Block) {
    for stmt in &block.stmts {
        if builder.is_terminated() { break; }
        lower_stmt(builder, stmt);
    }
}
