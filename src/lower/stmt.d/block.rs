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

    // `builder.locals` (nom → slot) est une unique table plate, pas une
    // pile de scopes comme côté sema (`crate::sema::scope::ScopeStack`) :
    // sans précaution, une variable déclarée dans CE bloc sous un nom déjà
    // utilisé par un bloc englobant écrase définitivement son mapping pour
    // le reste de la fonction, y compris après la fin de ce bloc — confirmé
    // par reproduction (y compris pour un simple `var`, pas seulement
    // `scoped`/`consumed` : use-after-free une fois la variable interne
    // libérée, voir docs/roadmap.d/memoire-double-free-et-fuites-scoped.md).
    // On restaure donc la vue d'avant ce bloc en sortie : les noms externes
    // masqués retrouvent leur slot d'origine, ceux déclarés ICI disparaissent.
    let locals_snapshot = builder.locals.clone();

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

    // Restaure la vue "avant ce bloc" (voir le commentaire au-dessus de
    // `locals_snapshot`) — indépendant de la terminaison : le code qui suit
    // ce bloc (bloc frère, code après un if/switch/boucle) a besoin de la
    // vue correcte, que CE chemin particulier ait terminé ou non.
    let declared_here: Vec<String> = builder.locals.keys()
        .filter(|k| !locals_snapshot.contains_key(k.as_str()))
        .cloned()
        .collect();
    for name in declared_here {
        builder.locals.remove(&name);
    }
    for (name, binding) in locals_snapshot {
        builder.locals.insert(name, binding);
    }

    builder.block_scope_stack.pop();
}
