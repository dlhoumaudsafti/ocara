/// Génère `__free_<Classe>`/`__clone_<Classe>` pour chaque classe
/// utilisateur — destruction/clonage réels des instances de classe pour
/// `scoped`/`consumed MaClasse` (voir docs/EBNF.md §9.2 et le plan "Gestion
/// de propriété des variables").
///
/// Portée :
///   - Les CHAMPS de type `string`/`array<T>`/`map<K,V>` ou une AUTRE classe
///     utilisateur (elle-même dans `class_field_types`, donc avec son propre
///     `__free_`/`__clone_` généré) sont libérés/clonés récursivement.
///   - Un champ de type ressource native (`Mutex`/`SQLite`/`MySQL`/
///     `MariaDB`/`HTTPRequest`/`HTTPResponse`) est FERMÉ (pas cloné — voir
///     `build_clone_function`) via son symbole runtime dédié
///     (`crate::sema::scope::resource_closer_symbol`) — un tel champ n'est
///     autorisé à la déclaration QUE parce que la classe porteuse est alors
///     traitée comme `OwnershipClass::Resource` partout où l'échappement est
///     vérifié (voir `crate::sema::scope::compute_resource_classes` et son
///     usage dans `crate::sema::typecheck`) : impossible de l'assigner/
///     retourner/passer en argument hors de son bloc `scoped`/`consumed`,
///     donc jamais deux instances vivantes ne peuvent se partager le même
///     handle natif. `Thread` reste hors périmètre (pas de destructeur
///     synthétisable, voir E19) — un champ `Thread` continue de tomber dans
///     `Plain` (jamais fermé), inchangé.
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
use crate::sema::scope::{ownership_class, resource_closer_symbol, OwnershipClass};

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
    /// `string`/`array<T>`/`map<K,V>` — symbole choisi selon le type déclaré
    /// (voir `crate::lower::stmt::ownership::value_ownership_symbol`).
    Value,
    /// Une autre classe utilisateur (a son propre `__free_`/`__clone_`).
    Object(String),
    /// Ressource native (`Mutex`/`SQLite`/`MySQL`/`MariaDB`/`HTTPRequest`/
    /// `HTTPResponse`) — fermée via son symbole runtime dédié à la
    /// libération, JAMAIS clonée (voir `build_clone_function`). Le `String`
    /// est le nom du type (ex. `"SQLite"`), pour retrouver le bon symbole
    /// via `resource_closer_symbol`.
    Resource(String),
    /// Primitif, `Thread`, type non pris en charge — copie brute (clone) ou
    /// rien (free) : la valeur elle-même n'est pas possédée par ce champ.
    Plain,
}

fn classify_field(ty: &Type, field_types: &HashMap<String, Vec<(String, Type)>>) -> FieldOwnership {
    match ty {
        Type::String | Type::Array(_) | Type::Map(_, _) => FieldOwnership::Value,
        Type::Named(n) if ownership_class(ty) == OwnershipClass::Resource => FieldOwnership::Resource(n.clone()),
        Type::Named(n) if field_types.contains_key(n) => FieldOwnership::Object(n.clone()),
        // Champ de type générique (`property box:Box<int>`) : le générique
        // monomorphisé est une classe utilisateur comme une autre dans
        // `field_types` (voir `crate::core::monomorph::monomorphize`) — sans
        // ce cas, `__free_<Classe>`/`__clone_<Classe>` traitaient un tel
        // champ comme `Plain` (copie brute au clone, jamais libéré),
        // confirmé par reproduction : voir docs/roadmap.d/langage-generiques.md.
        Type::Generic { name, args } => {
            let specialized = crate::core::monomorph::monomorphized_name(name, args);
            if field_types.contains_key(&specialized) {
                FieldOwnership::Object(specialized)
            } else {
                FieldOwnership::Plain
            }
        }
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

/// Champ `array<Classe>`/`map<K, Classe>` propriétaire de ses objets
/// (`IrModule::owning_fields`, voir `lower::stmt::object_facts`) : ses
/// instances sont libérées/clonées avec lui.
/// Le champ `fname` de `class_name` possède-t-il ses objets ?
fn owns_objects(module: &IrModule, class_name: &str, fname: &str) -> bool {
    module.field_decl.get(&format!("{}.{}", class_name, fname)).is_some_and(|k| module.owning_fields.contains(k))
}

fn emit_objects_ownership_call(module: &IrModule, f: &mut IrFunction, owner: &str, fname: &str, ty: &Type, clone: bool, v: &Value, dest: &Option<Value>) -> bool {
    let Some(class) = crate::lower::stmt::object_owners::object_elem_class(ty) else { return false };
    if !module.class_field_types.contains_key(class) {
        return false;
    }
    if !owns_objects(module, owner, fname) {
        return false;
    }
    let kind = if matches!(ty, Type::Map(..)) { "map" } else { "array" };
    let (op, elem) = if clone { ("clone", "__clone_") } else { ("free", "__free_") };
    let addr = f.new_value();
    f.emit(Inst::FuncAddr { dest: addr.clone(), func: format!("{}{}", elem, class) });
    let ret_ty = if clone { IrType::Ptr } else { IrType::Void };
    f.emit(Inst::Call { dest: dest.clone(), func: format!("__{}_{}_objects", kind, op), args: vec![v.clone(), addr], ret_ty });
    true
}

/// Champ conteneur d'objets NON propriétaire (partagé, ou propriété non
/// prouvée) : ni libéré ni dupliqué, l'objet n'en est pas le seul détenteur
/// (`use Bag(ys)` puis `ys` relu après la libération de l'objet était un
/// use-after-free).
fn is_shared_object_field(module: &IrModule, owner: &str, fname: &str, ty: &Type) -> bool {
    crate::lower::stmt::object_owners::object_elem_class(ty).is_some_and(|c| module.class_field_types.contains_key(c))
        && !owns_objects(module, owner, fname)
}

/// Appel de libération/clonage d'un champ `string`/`array`/`map` de type
/// `ty` sur la valeur `v` — `dest` pour un clonage.
fn emit_value_ownership_call(module: &mut IrModule, f: &mut IrFunction, owner: &str, fname: &str, ty: &Type, clone: bool, v: Value, dest: Option<Value>) {
    if emit_objects_ownership_call(module, f, owner, fname, ty, clone, &v, &dest) {
        return;
    }
    let Some((func, shape)) = crate::lower::stmt::ownership::value_ownership_symbol(ty, clone) else { return };
    let mut args = vec![v];
    if let Some(shape) = shape {
        let shape_val = f.new_value();
        f.emit(Inst::ConstStr { dest: shape_val.clone(), idx: module.intern_string(&shape) });
        let offset_val = f.new_value();
        f.emit(Inst::ConstInt { dest: offset_val.clone(), value: 0 });
        args.extend([shape_val, offset_val]);
    }
    let ret_ty = if clone { IrType::Ptr } else { IrType::Void };
    f.emit(Inst::Call { dest, func: func.to_string(), args, ret_ty });
}

/// `fn __free_<Classe>(obj: i64) -> void`
fn build_free_function(module: &mut IrModule, class_name: &str) -> IrFunction {
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
            FieldOwnership::Value if is_shared_object_field(module, class_name, fname, fty) => {}
            FieldOwnership::Value => {
                let v = f.new_value();
                f.emit(Inst::GetField { dest: v.clone(), obj: obj.clone(), field: fname.clone(), ty: ir_ty, offset });
                emit_value_ownership_call(module, &mut f, class_name, fname, fty, false, v, None);
            }
            FieldOwnership::Object(other_class) => {
                let v = f.new_value();
                f.emit(Inst::GetField { dest: v.clone(), obj: obj.clone(), field: fname.clone(), ty: ir_ty, offset });
                f.emit(Inst::Call { dest: None, func: format!("__free_{}", other_class), args: vec![v], ret_ty: IrType::Void });
            }
            FieldOwnership::Resource(ty_name) => {
                // `resource_closer_symbol` ne peut retourner `None` ici :
                // `classify_field` ne produit `Resource(n)` que pour un `n`
                // dont `ownership_class` vaut déjà `Resource`, exactement
                // l'ensemble couvert par `resource_closer_symbol` — les deux
                // fonctions partagent la même source de vérité
                // (`crate::sema::scope::ownership_class`). Le symbole
                // runtime lui-même est null-safe (voir `SQLite_close`/
                // `Mutex_destroy`, `runtime/src/*.rs`), pas besoin de garde
                // supplémentaire ici.
                if let Some(closer) = resource_closer_symbol(&ty_name) {
                    let v = f.new_value();
                    f.emit(Inst::GetField { dest: v.clone(), obj: obj.clone(), field: fname.clone(), ty: ir_ty, offset });
                    f.emit(Inst::Call { dest: None, func: closer.to_string(), args: vec![v], ret_ty: IrType::Void });
                }
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
fn build_clone_function(module: &mut IrModule, class_name: &str) -> IrFunction {
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
            FieldOwnership::Value if is_shared_object_field(module, class_name, fname, fty) => src,
            FieldOwnership::Value => {
                let cloned = f.new_value();
                emit_value_ownership_call(module, &mut f, class_name, fname, fty, true, src, Some(cloned.clone()));
                cloned
            }
            FieldOwnership::Object(other_class) => {
                let cloned = f.new_value();
                f.emit(Inst::Call { dest: Some(cloned.clone()), func: format!("__clone_{}", other_class), args: vec![src], ret_ty: IrType::Ptr });
                cloned
            }
            // Un handle de ressource ne peut PAS être dupliqué (rouvrir la
            // même connexion/le même verrou n'a pas de sens) ni partagé tel
            // quel (`__free_<Classe>` fermerait le handle sous le nez de
            // l'autre instance — double-free/use-after-close silencieux).
            // En pratique, ce cas ne devrait JAMAIS être atteint par un
            // programme qui compile : la classe porteuse est traitée comme
            // `OwnershipClass::Resource` (voir `compute_resource_classes`),
            // donc `check_escape`/`check_argument_escape` rejettent tout
            // échappement d'une `scoped`/`consumed` de cette classe AVANT
            // que `__clone_<Classe>` ne soit jamais émis pour elle — garde
            // défensive seulement : un champ neuf à 0 (jamais assigné) plutôt
            // qu'un pointeur partagé, si ce code était atteint malgré tout.
            FieldOwnership::Resource(_) => {
                let zero = f.new_value();
                f.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
                zero
            }
            // Primitif/`Thread`/non pris en charge : copie brute de la
            // valeur (int/float/bool réels ; un pointeur `Thread` est
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
