//! Bilan des fuites (`--leak-check`) : `main` appelle d'abord
//! `__leak_check_init(noms)`, qui active le comptage des allocations du
//! runtime (runtime/src/leak.rs) ; `noms` est la table statique
//! `id=Classe;…` des classes du programme, pour nommer les objets restants.

use crate::ir::inst::Inst;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;

pub fn instrument(module: &mut IrModule) {
    let mut ids: Vec<(&String, &i64)> = module.class_ids.iter().collect();
    ids.sort_by_key(|(_, id)| **id);
    let table: String = ids.iter()
        .map(|(name, id)| format!("{}={}", id, crate::core::analysis::homonyms::display(name)))
        .collect::<Vec<_>>()
        .join(";");
    let idx = module.intern_string(&table);
    let Some(main) = module.functions.iter_mut().find(|f| f.name == "main") else { return };
    let names = main.new_value();
    let Some(entry) = main.blocks.first_mut() else { return };
    entry.insts.insert(0, Inst::Call { dest: None, func: "__leak_check_init".into(), args: vec![names.clone()], ret_ty: IrType::Void });
    entry.insts.insert(0, Inst::ConstStr { dest: names, idx });
}
