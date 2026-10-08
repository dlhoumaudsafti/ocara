//! Descripteurs de classe du comptage de références
//! (docs/roadmap.d/memoire-refcount.md) : un masque `'0'`/`'1'` par classe,
//! un caractère par champ, `'1'` pour un champ qui porte une valeur tas.
//! Le masque, interné comme chaîne littérale, est rangé dans l'en-tête de
//! chaque instance par `__alloc_class_obj`.

use std::collections::HashSet;

use crate::ir::module::IrModule;
use crate::parsing::ast::{Program, Type};

/// Types dont la valeur est soit nulle, soit une valeur tas munie d'un
/// en-tête (ou boxée) : jamais un scalaire brut.
pub(crate) fn is_counted(ty: &Type, objects: &HashSet<String>) -> bool {
    match ty {
        Type::String | Type::Mixed | Type::Array(_) | Type::Map(..) | Type::Function { .. } | Type::Generic { .. } => true,
        Type::Named(n) => objects.contains(n),
        Type::Qualified(parts) => parts.last().is_some_and(|n| objects.contains(n)),
        Type::Union(members) => members.iter().all(|m| matches!(m, Type::Null) || is_counted(m, objects)),
        _ => false,
    }
}

/// Classes dont les valeurs sont des instances munies d'un en-tête : classes
/// et interfaces du programme, exceptions builtin. Les autres classes
/// builtin peuvent être des pointeurs Rust nus : jamais comptées.
pub fn compute_rc_objects(module: &mut IrModule, program: &Program) {
    let exception = module.class_layouts.get("Exception").cloned();
    let exceptions: Vec<String> = module.class_layouts.iter()
        .filter(|(_, layout)| Some(*layout) == exception.as_ref())
        .map(|(name, _)| name.clone())
        .collect();
    module.rc_objects = program.classes.iter().map(|c| c.name.clone())
        .chain(program.interfaces.iter().map(|i| i.name.clone()))
        .chain(exceptions)
        .collect();
}

fn mask_for(module: &IrModule, class: &str, objects: &HashSet<String>) -> String {
    let layout = &module.class_layouts[class];
    let types = module.class_field_types.get(class);
    layout.iter().map(|(field, _)| {
        let declared = types.and_then(|types| types.iter().find(|(f, _)| f == field));
        let counted = match declared {
            Some((_, ty)) => is_counted(ty, objects),
            None => is_builtin_exception_string(module, class, field),
        };
        if counted { '1' } else { '0' }
    }).collect()
}

/// Champ `message`/`source` d'une exception builtin, ou hérité d'elle par
/// une classe utilisateur (la disposition commence par celle d'`Exception`).
fn is_builtin_exception_string(module: &IrModule, class: &str, field: &str) -> bool {
    module.class_layouts.get("Exception").is_some_and(|l| module.class_layouts[class].starts_with(l))
        && matches!(field, "message" | "source")
}

/// Calcule `IrModule::class_masks` pour toutes les classes allouées par
/// `__alloc_class_obj` (les blocs internes `__env_*`/`__gen_*` n'en ont pas).
pub fn compute_class_masks(module: &mut IrModule) {
    let objects = module.rc_objects.clone();
    let classes: Vec<String> = module.class_layouts.keys().filter(|c| !c.starts_with("__")).cloned().collect();
    for class in classes {
        let mask = mask_for(module, &class, &objects);
        let idx = module.intern_string(&mask);
        module.class_masks.insert(class, idx);
    }
}
