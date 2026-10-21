//! Table des classes du programme pour le runtime : `id=Classe|Parent|…|
//! Interface…;…`, enregistrée en tête de `main` (`__class_table_init`). Sert
//! à `Convert::mixedAs` (runtime/src/mixed_check.rs) : une valeur `mixed`
//! rangée dans une variable de classe `U` doit être une instance de `U`,
//! d'une sous-classe, ou d'une classe qui implémente l'interface `U`.

use crate::ir::inst::Inst;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use crate::parsing::ast::Program;

pub fn register(module: &mut IrModule, program: &Program) {
    let mut entries: Vec<(i64, String)> = program.classes.iter()
        .filter_map(|class| Some((*module.class_ids.get(&class.name)?, kinds_of(program, &class.name).join("|"))))
        .collect();
    entries.sort();
    let table: String = entries.iter().map(|(id, kinds)| format!("{}={}", id, kinds)).collect::<Vec<_>>().join(";");
    let idx = module.intern_string(&table);
    let Some(main) = module.functions.iter_mut().find(|f| f.name == "main") else { return };
    let text = main.new_value();
    let Some(entry) = main.blocks.first_mut() else { return };
    entry.insts.insert(0, Inst::Call { dest: None, func: "__class_table_init".into(), args: vec![text.clone()], ret_ty: IrType::Void });
    entry.insts.insert(0, Inst::ConstStr { dest: text, idx });
}

/// La classe, ses ancêtres et les interfaces qu'ils implémentent.
fn kinds_of(program: &Program, class: &str) -> Vec<String> {
    let mut kinds = Vec::new();
    let mut current = program.classes.iter().find(|c| c.name == class);
    while let Some(c) = current {
        if kinds.contains(&c.name) {
            break;
        }
        kinds.push(c.name.clone());
        kinds.extend(c.implements.iter().cloned());
        current = c.extends.as_ref().and_then(|p| program.classes.iter().find(|c| &c.name == p));
    }
    kinds
}
