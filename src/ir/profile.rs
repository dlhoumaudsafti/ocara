//! Instrumentation de profilage (`--profile`) : chaque fonction et méthode
//! du programme (hors fonctions internes `__…`) appelle `__prof_enter(nom)`
//! en entrée et `__prof_exit(nom)` avant chaque `return` — mesures tenues
//! par le runtime (runtime/src/profile.rs). Nom affiché : `Classe.méthode`
//! ou `fonction`.

use crate::ir::func::IrFunction;
use crate::ir::inst::Inst;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;

pub fn instrument(module: &mut IrModule, classes: &[String]) {
    let names: Vec<String> = module.functions.iter().map(|f| f.name.clone()).collect();
    let labels: Vec<Option<u32>> = names.iter()
        .map(|name| (!name.starts_with("__")).then(|| module.intern_string(&label(name, classes))))
        .collect();
    for (func, idx) in module.functions.iter_mut().zip(labels) {
        if let Some(idx) = idx {
            wrap(func, idx);
        }
    }
}

fn wrap(func: &mut IrFunction, idx: u32) {
    let mut calls: Vec<(usize, usize, &'static str)> = Vec::new();
    calls.push((0, 0, "__prof_enter"));
    for (b, block) in func.blocks.iter().enumerate() {
        for (i, inst) in block.insts.iter().enumerate() {
            if matches!(inst, Inst::Return { .. }) {
                calls.push((b, i, "__prof_exit"));
            }
        }
    }
    // Insertion de la fin vers le début : les positions restent valides.
    for (b, i, hook) in calls.into_iter().rev() {
        let name = func.new_value();
        let insts = &mut func.blocks[b].insts;
        insts.insert(i, Inst::Call { dest: None, func: hook.into(), args: vec![name.clone()], ret_ty: IrType::Void });
        insts.insert(i, Inst::ConstStr { dest: name, idx });
    }
}

/// `Classe_méthode` → `Classe.méthode` (classe la plus longue qui préfixe le
/// nom), sinon le nom tel quel.
fn label(name: &str, classes: &[String]) -> String {
    classes.iter()
        .filter(|c| name.len() > c.len() + 1 && name.starts_with(c.as_str()) && name.as_bytes()[c.len()] == b'_')
        .max_by_key(|c| c.len())
        .map(|c| format!("{}.{}", c, &name[c.len() + 1..]))
        .unwrap_or_else(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::label;

    #[test]
    fn method_names_are_split_on_their_class() {
        let classes = vec!["Car".to_string(), "CarView".to_string()];
        assert_eq!(label("CarView_show", &classes), "CarView.show");
        assert_eq!(label("Car_init", &classes), "Car.init");
        assert_eq!(label("compute_total", &classes), "compute_total");
    }
}
