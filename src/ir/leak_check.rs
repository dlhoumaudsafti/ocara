//! Bilan des fuites (`--leak-check`) : `main` appelle d'abord
//! `__leak_check_init(noms)`, qui active le comptage des allocations du
//! runtime (runtime/src/leak.rs) ; `noms` est la table statique
//! `id=Classe;…` des classes du programme. Chaque fonction est encadrée de
//! `__leak_enter(lieu)`/`__leak_exit(lieu)` (`lieu` : `nom␟fichier␟ligne`),
//! pour rapporter où chaque valeur restante a été allouée.

use std::collections::HashMap;

use crate::ir::inst::Inst;
use crate::ir::module::IrModule;
use crate::ir::profile::{label, wrap};
use crate::ir::types::IrType;

/// `sites` : nom IR d'une fonction → (fichier, ligne) de sa déclaration.
pub fn instrument(module: &mut IrModule, classes: &[String], sites: &HashMap<String, (String, usize)>) {
    let mut ids: Vec<(&String, &i64)> = module.class_ids.iter().collect();
    ids.sort_by_key(|(_, id)| **id);
    let table: String = ids.iter()
        .map(|(name, id)| format!("{}={}", id, crate::core::analysis::homonyms::display(name)))
        .collect::<Vec<_>>()
        .join(";");
    let names: Vec<String> = module.functions.iter().map(|f| f.name.clone()).collect();
    let site_ids: Vec<Option<u32>> = names.iter()
        .map(|name| (!name.starts_with("__")).then(|| {
            let (file, line) = sites.get(name).cloned().unwrap_or_default();
            let shown = crate::core::analysis::homonyms::display(&label(name, classes));
            module.intern_string(&format!("{}\u{1f}{}\u{1f}{}", shown, file, line))
        }))
        .collect();
    for (func, idx) in module.functions.iter_mut().zip(site_ids) {
        if let Some(idx) = idx {
            wrap(func, idx, "__leak_enter", "__leak_exit");
        }
    }
    let table_idx = module.intern_string(&table);
    let Some(main) = module.functions.iter_mut().find(|f| f.name == "main") else { return };
    let names = main.new_value();
    let Some(entry) = main.blocks.first_mut() else { return };
    entry.insts.insert(0, Inst::Call { dest: None, func: "__leak_check_init".into(), args: vec![names.clone()], ret_ty: IrType::Void });
    entry.insts.insert(0, Inst::ConstStr { dest: names, idx: table_idx });
}
