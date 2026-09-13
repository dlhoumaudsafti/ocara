/// Lowering des structures de contrôle (if/switch/while)

use crate::parsing::ast::*;
use crate::ir::types::IrType;
use crate::ir::inst::Inst;
use crate::lower::builder::LowerBuilder;
use crate::lower::expr::lower_expr;
use super::super::super::block::lower_block;

/// Lowering récursif de la chaîne elseif.
///
/// `outer_snapshot` : état de `owned_locals` avant tout le if/elseif/else —
/// restauré après le dernier bras de la chaîne (voir la doc de `lower_if`
/// pour la justification de ces sauvegardes/restaurations).
fn lower_elseif_chain(
    builder:    &mut LowerBuilder,
    elseif:     &[(Expr, Block)],
    else_block: Option<&Block>,
    merge_bb:   &crate::ir::inst::BlockId,
    outer_snapshot: &std::collections::HashMap<String, crate::lower::stmt::ownership::OwnedLocalInfo>,
) {
    if elseif.is_empty() {
        if let Some(blk) = else_block {
            lower_block(builder, blk);
        }
        if !builder.is_terminated() {
            builder.emit(Inst::Jump { target: merge_bb.clone() });
        }
        builder.owned_locals = outer_snapshot.clone();
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
    // Ce bras (elseif) est mutuellement exclusif avec les suivants de la
    // chaîne : une destruction anticipée (return/break/continue) faite ici
    // ne doit jamais empêcher un bras suivant de détruire à son tour la
    // même variable sur son propre chemin d'exécution (voir la doc de
    // `lower_if`).
    builder.owned_locals = outer_snapshot.clone();

    builder.switch_to(&next_bb);
    lower_elseif_chain(builder, &elseif[1..], else_block, merge_bb, outer_snapshot);
}

/// Lowering d'un `if`/`elseif`/`else`.
///
/// Chaque branche (`then`, chaque `elseif`, `else`) est mutuellement
/// exclusive à l'exécution : une seule s'exécute réellement. Or
/// `owned_locals` (voir `crate::lower::stmt::ownership::OwnedLocalInfo`) est
/// un unique état partagé pendant tout le lowering de la fonction — sans
/// précaution, une destruction anticipée (`return`/`break`/`continue`) émise
/// dans une branche marquerait la variable "déjà détruite" pour les branches
/// suivantes ET pour le code après le if, qui suivent pourtant un chemin
/// d'exécution différent et n'ont jamais réellement vu cette destruction :
/// la variable fuirait sur leur propre chemin (confirmé par reproduction —
/// voir `docs/roadmap.d/memoire-double-free-et-fuites-scoped.md`).
///
/// Chaque branche reçoit donc son propre instantané de `owned_locals` pris
/// juste avant elle, restauré juste après — ainsi une branche ne voit jamais
/// les effets d'une autre. Après la dernière branche (elseif/else compris),
/// l'état est restauré à celui d'avant le if tout entier : seule une branche
/// qui termine réellement (et qui donc n'atteint jamais le point de fusion)
/// peut avoir modifié quoi que ce soit — le code après le if doit se
/// comporter comme si aucune branche n'avait rien changé.
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

    let outer_snapshot = builder.owned_locals.clone();

    // Then
    builder.switch_to(&then_bb);
    lower_block(builder, then_block);
    if !builder.is_terminated() {
        builder.emit(Inst::Jump { target: merge_bb.clone() });
    }
    builder.owned_locals = outer_snapshot.clone();

    // Elseif / Else
    builder.switch_to(&else_bb);
    if !elseif.is_empty() {
        // Lowering de la chaîne elseif de manière récursive — restaure déjà
        // `outer_snapshot` en interne après son dernier bras.
        lower_elseif_chain(builder, elseif, else_block.as_ref(), &merge_bb, &outer_snapshot);
    } else if let Some(blk) = else_block {
        lower_block(builder, blk);
        if !builder.is_terminated() {
            builder.emit(Inst::Jump { target: merge_bb.clone() });
        }
        builder.owned_locals = outer_snapshot;
    } else {
        builder.emit(Inst::Jump { target: merge_bb.clone() });
        builder.owned_locals = outer_snapshot;
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

    // Chaque `case` (et `default`) est mutuellement exclusif — même
    // précaution que `lower_if` (voir sa doc) : chacun reçoit son propre
    // instantané de `owned_locals`, restauré après lui, pour qu'une
    // destruction anticipée dans l'un ne fuite pas vers les autres ni vers
    // le code après le switch.
    let outer_snapshot = builder.owned_locals.clone();

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
        builder.owned_locals = outer_snapshot.clone();

        builder.switch_to(&next_bb);
    }

    if let Some(blk) = default {
        lower_block(builder, blk);
    }
    if !builder.is_terminated() {
        builder.emit(Inst::Jump { target: merge_bb.clone() });
    }
    builder.owned_locals = outer_snapshot;

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
    builder.loop_stack.push((cond_bb.clone(), merge_bb.clone(), builder.block_scope_stack.len()));
    builder.loop_depth += 1;
    lower_block(builder, body);
    builder.loop_depth -= 1;
    builder.loop_stack.pop();
    if !builder.is_terminated() {
        builder.emit(Inst::Jump { target: cond_bb.clone() });
    }

    builder.switch_to(&merge_bb);
}
