/// Génère `__free_<Classe>`/`__clone_<Classe>` pour chaque classe
/// utilisateur — destruction/clonage réels des instances de classe pour
/// `scoped`/`consumed MaClasse` (voir docs/EBNF.md §9.2 et le plan "Gestion
/// de propriété des variables").
///
/// Portée volontairement limitée (cohérent avec le reste du chantier) :
///   - Seuls les CHAMPS de type `string`/`array<T>`/`map<K,V>` ou une AUTRE
///     classe utilisateur (elle-même dans `class_field_types`, donc avec
///     son propre `__free_`/`__clone_` généré) sont libérés/clonés
///     récursivement. Un champ de type ressource (`Mutex`, `Thread`, ...)
///     n'est PAS libéré ici — fuite, pas un crash, cohérent avec le fait
///     que ce chantier ne synthétise de destructeur automatique QUE pour
///     `scoped`/`consumed` directement sur ces types, pas pour un champ.
///   - Les classes builtin/opaques (Mutex, SDL, Exception, ...) n'ont pas
///     d'entrée dans `class_field_types` (voir sa doc) : aucune fonction
///     n'est générée pour elles. `has_generated_destructor` est le point de
///     vérité unique pour savoir si `scoped`/`consumed` sur une classe
///     donnée a réellement un `__free_`/`__clone_` à appeler — ne jamais
///     émettre un appel vers l'un de ces deux sans passer par cette
///     vérification, sous peine d'appeler un symbole qui n'existe pas.
use std::collections::HashMap;

use crate::ir::func::{IrFunction, IrParam};
use crate::ir::inst::{Inst, Value, BlockId};
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use crate::parsing::ast::{Program, Type};

/// Vrai si `class_name` a (ou aura, dans le même passage) un
/// `__free_<class_name>`/`__clone_<class_name>` généré — c'est-à-dire une
/// classe utilisateur réelle (présente dans `program.classes`), pas un
/// builtin/opaque. Seul point de vérité à consulter avant d'émettre un
/// appel vers l'une de ces deux fonctions (voir `crate::lower::stmt::ownership`).
pub fn has_generated_destructor(module: &IrModule, class_name: &str) -> bool {
    module.class_field_types.contains_key(class_name)
}

/// Ce qu'il faut faire d'un champ lors de la libération/du clonage de
/// l'objet qui le porte.
enum FieldOwnership {
    /// `string`/`array<T>`/`map<K,V>` — via `__value_free`/`__value_clone`.
    Value,
    /// Une autre classe utilisateur (a son propre `__free_`/`__clone_`).
    Object(String),
    /// Primitif, ressource, type non pris en charge — copie brute (clone)
    /// ou rien (free) : la valeur elle-même n'est pas possédée par ce champ.
    Plain,
}

fn classify_field(ty: &Type, field_types: &HashMap<String, Vec<(String, Type)>>) -> FieldOwnership {
    match ty {
        Type::String | Type::Array(_) | Type::Map(_, _) => FieldOwnership::Value,
        Type::Named(n) if field_types.contains_key(n) => FieldOwnership::Object(n.clone()),
        _ => FieldOwnership::Plain,
    }
}

/// Génère `__free_<Classe>`/`__clone_<Classe>` pour chaque classe de
/// `program.classes` et les ajoute à `module`. Doit être appelé APRÈS que
/// `module.class_layouts`/`module.class_field_types` sont entièrement
/// peuplés pour TOUTES les classes (l'ordre entre classes n'a pas
/// d'importance au-delà de ça — voir la doc du site d'appel).
pub fn generate_class_ownership_functions(module: &mut IrModule, program: &Program) {
    for class in &program.classes {
        let free_fn = build_free_function(module, &class.name);
        module.add_function(free_fn);
        let clone_fn = build_clone_function(module, &class.name);
        module.add_function(clone_fn);
    }
}

/// `fn __free_<Classe>(obj: i64) -> void`
fn build_free_function(module: &IrModule, class_name: &str) -> IrFunction {
    let name = format!("__free_{}", class_name);
    let mut f = IrFunction::new(name, vec![], IrType::Void);
    let obj = f.new_value();
    f.params = vec![IrParam { name: "obj".into(), ty: IrType::Ptr, slot: obj.clone() }];

    // obj == 0 → rien à faire (garde-fou, cohérent avec __array_free/__map_free).
    let (body_bb, end_bb) = emit_null_guard(&mut f, &obj);
    f.switch_to(&body_bb);

    let field_types = module.class_field_types.get(class_name).cloned().unwrap_or_default();
    let field_ir_layout = module.class_layouts.get(class_name).cloned().unwrap_or_default();
    for (idx, (fname, fty)) in field_types.iter().enumerate() {
        let offset = (idx * 8) as i32;
        let ir_ty = field_ir_layout.get(idx).map(|(_, t)| t.clone()).unwrap_or(IrType::Ptr);
        match classify_field(fty, &module.class_field_types) {
            FieldOwnership::Value => {
                let v = f.new_value();
                f.emit(Inst::GetField { dest: v.clone(), obj: obj.clone(), field: fname.clone(), ty: ir_ty, offset });
                f.emit(Inst::Call { dest: None, func: "__value_free".into(), args: vec![v], ret_ty: IrType::Void });
            }
            FieldOwnership::Object(other_class) => {
                let v = f.new_value();
                f.emit(Inst::GetField { dest: v.clone(), obj: obj.clone(), field: fname.clone(), ty: ir_ty, offset });
                f.emit(Inst::Call { dest: None, func: format!("__free_{}", other_class), args: vec![v], ret_ty: IrType::Void });
            }
            FieldOwnership::Plain => {}
        }
    }

    // Libère enfin le bloc de l'objet lui-même.
    let n_fields = field_ir_layout.len() as i64;
    let n_fields_val = f.new_value();
    f.emit(Inst::ConstInt { dest: n_fields_val.clone(), value: n_fields });
    f.emit(Inst::Call { dest: None, func: "__object_free".into(), args: vec![obj, n_fields_val], ret_ty: IrType::Void });
    f.emit(Inst::Return { value: None });

    f.switch_to(&end_bb);
    f.emit(Inst::Return { value: None });
    f
}

/// `fn __clone_<Classe>(obj: i64) -> i64`
fn build_clone_function(module: &IrModule, class_name: &str) -> IrFunction {
    let name = format!("__clone_{}", class_name);
    let mut f = IrFunction::new(name, vec![], IrType::Ptr);
    let obj = f.new_value();
    f.params = vec![IrParam { name: "obj".into(), ty: IrType::Ptr, slot: obj.clone() }];

    // obj == 0 → retourne 0 tel quel (rien à cloner).
    let (body_bb, end_bb) = emit_null_guard(&mut f, &obj);
    f.switch_to(&body_bb);

    // Nouvel objet, même classe (réutilise Inst::Alloc — même calcul de
    // taille que le codegen utilise déjà pour `use Classe(...)`).
    let new_obj = f.new_value();
    f.emit(Inst::Alloc { dest: new_obj.clone(), class: class_name.to_string() });

    let field_types = module.class_field_types.get(class_name).cloned().unwrap_or_default();
    let field_ir_layout = module.class_layouts.get(class_name).cloned().unwrap_or_default();
    for (idx, (fname, fty)) in field_types.iter().enumerate() {
        let offset = (idx * 8) as i32;
        let ir_ty = field_ir_layout.get(idx).map(|(_, t)| t.clone()).unwrap_or(IrType::Ptr);
        let src = f.new_value();
        f.emit(Inst::GetField { dest: src.clone(), obj: obj.clone(), field: fname.clone(), ty: ir_ty.clone(), offset });
        let to_store = match classify_field(fty, &module.class_field_types) {
            FieldOwnership::Value => {
                let cloned = f.new_value();
                f.emit(Inst::Call { dest: Some(cloned.clone()), func: "__value_clone".into(), args: vec![src], ret_ty: IrType::Ptr });
                cloned
            }
            FieldOwnership::Object(other_class) => {
                let cloned = f.new_value();
                f.emit(Inst::Call { dest: Some(cloned.clone()), func: format!("__clone_{}", other_class), args: vec![src], ret_ty: IrType::Ptr });
                cloned
            }
            // Primitif/ressource/non pris en charge : copie brute de la
            // valeur (int/float/bool réels ; un pointeur de ressource est
            // partagé tel quel — pas dupliqué, cohérent avec le fait qu'on
            // ne génère pas de destructeur pour ce genre de champ non plus).
            FieldOwnership::Plain => src,
        };
        f.emit(Inst::SetField { obj: new_obj.clone(), field: fname.clone(), src: to_store, offset });
    }
    f.emit(Inst::Return { value: Some(new_obj) });

    f.switch_to(&end_bb);
    let zero_ret = f.new_value();
    f.emit(Inst::ConstInt { dest: zero_ret.clone(), value: 0 });
    f.emit(Inst::Return { value: Some(zero_ret) });
    f
}

/// Émet `if obj == 0 { goto end } else { goto body }`, retourne (body_bb,
/// end_bb) — `end_bb` reste à compléter par l'appelant (un seul `Return`).
fn emit_null_guard(f: &mut IrFunction, obj: &Value) -> (BlockId, BlockId) {
    let zero = f.new_value();
    f.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
    let is_null = f.new_value();
    f.emit(Inst::CmpEq { dest: is_null.clone(), lhs: obj.clone(), rhs: zero, ty: IrType::I64 });
    let body_bb = f.new_block();
    let end_bb = f.new_block();
    f.emit(Inst::Branch { cond: is_null, then_bb: end_bb.clone(), else_bb: body_bb.clone() });
    (body_bb, end_bb)
}
