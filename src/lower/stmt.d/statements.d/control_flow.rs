/// Lowering des structures de contrôle (if/switch/while)

use crate::parsing::ast::*;
use crate::ir::types::IrType;
use crate::ir::inst::Inst;
use crate::lower::builder::LowerBuilder;
use crate::lower::expr::lower_expr;
use super::super::super::block::lower_block;

/// Lowering récursif de la chaîne elseif
fn lower_elseif_chain(
    builder:    &mut LowerBuilder,
    elseif:     &[(Expr, Block)],
    else_block: Option<&Block>,
    merge_bb:   &crate::ir::inst::BlockId,
) {
    if elseif.is_empty() {
        if let Some(blk) = else_block {
            lower_block(builder, blk);
        }
        if !builder.is_terminated() {
            builder.emit(Inst::Jump { target: merge_bb.clone() });
        }
        return;
    }

    let (cond_expr, then_blk) = &elseif[0];
    let cond_val = lower_expr(builder, cond_expr);
    let then_bb  = builder.new_block();
    let next_bb  = builder.new_block();

    builder.emit(Inst::Branch {
        cond:    cond_val,
        then_bb: then_bb.clone(),
        else_bb: next_bb.clone(),
    });

    builder.switch_to(&then_bb);
    lower_block(builder, then_blk);
    if !builder.is_terminated() {
        builder.emit(Inst::Jump { target: merge_bb.clone() });
    }

    builder.switch_to(&next_bb);
    lower_elseif_chain(builder, &elseif[1..], else_block, merge_bb);
}

pub fn lower_if(
    builder: &mut LowerBuilder,
    condition: &Expr,
    then_block: &Block,
    elseif: &[(Expr, Block)],
    else_block: &Option<Block>,
) {
    let cond_val = lower_expr(builder, condition);
    let then_bb  = builder.new_block();
    let else_bb  = builder.new_block();
    let merge_bb = builder.new_block();

    builder.emit(Inst::Branch {
        cond:    cond_val,
        then_bb: then_bb.clone(),
        else_bb: else_bb.clone(),
    });

    // Then
    builder.switch_to(&then_bb);
    lower_block(builder, then_block);
    if !builder.is_terminated() {
        builder.emit(Inst::Jump { target: merge_bb.clone() });
    }

    // Elseif / Else
    builder.switch_to(&else_bb);
    if !elseif.is_empty() {
        // Lowering de la chaîne elseif de manière récursive
        lower_elseif_chain(builder, elseif, else_block.as_ref(), &merge_bb);
    } else if let Some(blk) = else_block {
        lower_block(builder, blk);
        if !builder.is_terminated() {
            builder.emit(Inst::Jump { target: merge_bb.clone() });
        }
    } else {
        builder.emit(Inst::Jump { target: merge_bb.clone() });
    }

    builder.switch_to(&merge_bb);
}

pub fn lower_switch(
    builder: &mut LowerBuilder,
    subject: &Expr,
    cases: &[SwitchCase],
    default: &Option<Block>,
) {
    let subj = lower_expr(builder, subject);
    let merge_bb = builder.new_block();

    for case in cases {
        let body_bb = builder.new_block();
        let next_bb = builder.new_block();

        let pat_val = lower_expr(
            builder,
            &Expr::Literal(case.pattern.clone(), case.span.clone()),
        );
        let test = builder.new_value();
        builder.emit(Inst::CmpEq {
            dest: test.clone(),
            lhs:  subj.clone(),
            rhs:  pat_val,
            ty:   IrType::I64,
        });
        builder.emit(Inst::Branch {
            cond:    test,
            then_bb: body_bb.clone(),
            else_bb: next_bb.clone(),
        });

        builder.switch_to(&body_bb);
        lower_block(builder, &case.body);
        if !builder.is_terminated() {
            builder.emit(Inst::Jump { target: merge_bb.clone() });
        }

        builder.switch_to(&next_bb);
    }

    if let Some(blk) = default {
        lower_block(builder, blk);
    }
    if !builder.is_terminated() {
        builder.emit(Inst::Jump { target: merge_bb.clone() });
    }

    builder.switch_to(&merge_bb);
}

pub fn lower_while(
    builder: &mut LowerBuilder,
    condition: &Expr,
    body: &Block,
) {
    let cond_bb  = builder.new_block();
    let body_bb  = builder.new_block();
    let merge_bb = builder.new_block();

    builder.emit(Inst::Jump { target: cond_bb.clone() });
    builder.switch_to(&cond_bb);

    let cond_val = lower_expr(builder, condition);
    builder.emit(Inst::Branch {
        cond:    cond_val,
        then_bb: body_bb.clone(),
        else_bb: merge_bb.clone(),
    });

    builder.switch_to(&body_bb);
    // continue → cond_bb (réévalue la condition), break → merge_bb
    builder.loop_stack.push((cond_bb.clone(), merge_bb.clone()));
    lower_block(builder, body);
    builder.loop_stack.pop();
    if !builder.is_terminated() {
        builder.emit(Inst::Jump { target: cond_bb.clone() });
    }

    builder.switch_to(&merge_bb);
}
