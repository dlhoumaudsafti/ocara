/// Dispatch dynamique réel pour l'héritage de classe (voir
/// docs/roadmap.d/langage-interfaces.md et `super::interfaces`, le même
/// mécanisme pour les interfaces).
///
/// `var s:Shape = use Circle()` (avec `Circle extends Shape`) compilait déjà
/// (résolution statique de `Shape_describe`) mais appelait TOUJOURS
/// l'implémentation de `Shape`, jamais celle — potentiellement surchargée —
/// de la classe réelle de l'objet. Contrairement aux interfaces
/// (`Interface_method` n'existait jamais), `Shape_describe` EST une vraie
/// fonction (l'implémentation concrète de `Shape`, aussi utilisée par
/// `self`/`parent` et par la copie de méthode héritée — voir `lower_class`)
/// : on ne peut pas la remplacer par un dispatcher sans casser ces usages.
/// Un dispatcher séparé est donc généré sous un nom distinct,
/// `__dispatch_Classe_méthode`, et c'est le SITE D'APPEL externe
/// (`obj.method()`/`field.method()`, jamais `self`/`parent`) qui est
/// redirigé vers lui — voir `src/lower/expr.d/lower.rs`.
///
/// Limite assumée, non traitée par cette passe : un appel `self.method()`
/// DEPUIS le corps d'une méthode (y compris une copie héritée) reste résolu
/// STATIQUEMENT sur la classe sous laquelle ce corps est lowered, jamais
/// redirigé dynamiquement vers le type réel de l'objet — seuls les appels
/// EXTERNES bénéficient de ce dispatch. Un polymorphisme complet (dispatch
/// virtuel même pour les appels internes `self.foo()`) est un chantier plus
/// profond, hors de cette passe.
use std::collections::HashSet;
use crate::ir::func::{IrFunction, IrParam};
use crate::ir::inst::Inst;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use crate::parsing::ast::{ClassDecl, ClassMember, FuncDecl, Program};

/// Remplit `module.classes_with_subclasses` — calcul PUREMENT basé sur l'AST
/// (`program.classes`, indépendant de tout lowering), à appeler TRÈS TÔT
/// (avant le lowering du moindre corps de fonction/méthode) puisque
/// `class_dispatcher_name` y est consulté à CHAQUE site d'appel de méthode
/// pour décider s'il faut rediriger vers un dispatcher dynamique. Séparé de
/// `generate_class_dispatchers` (qui génère les CORPS des dispatchers eux-
/// mêmes, appelée bien plus tard une fois les méthodes concrètes lowered)
/// pour cette seule raison d'ordonnancement.
pub fn compute_classes_with_subclasses(module: &mut IrModule, program: &Program) {
    for class in &program.classes {
        if !transitive_descendants(&class.name, &program.classes).is_empty() {
            module.classes_with_subclasses.insert(class.name.clone());
        }
    }
}

/// Nom du dispatcher dynamique pour `class_name.method_name()` — `None` si
/// `class_name` n'a aucune sous-classe (dispatch inutile, l'appel direct à
/// `Classe_méthode` suffit et reste inchangé).
pub fn class_dispatcher_name(module: &IrModule, class_name: &str, method_name: &str) -> Option<String> {
    if !module.classes_with_subclasses.contains(class_name) {
        return None;
    }
    Some(format!("__dispatch_{}_{}", class_name, method_name))
}

/// Génère, pour chaque classe qui a au moins une sous-classe (directe ou
/// transitive), un dispatcher par méthode D'INSTANCE appelable (propre ou
/// héritée — jamais statique, jamais le constructeur : ni l'une ni l'autre
/// ne sont polymorphes).
///
/// Suppose `module.classes_with_subclasses` déjà rempli (voir
/// `compute_classes_with_subclasses`, appelée bien plus tôt — AVANT le
/// lowering de tout corps de fonction, puisque `class_dispatcher_name` y est
/// consulté à chaque site d'appel de méthode).
pub fn generate_class_dispatchers(module: &mut IrModule, program: &Program) {
    for class in &program.classes {
        let descendants = transitive_descendants(&class.name, &program.classes);
        if descendants.is_empty() {
            continue;
        }

        let mut candidates: Vec<&ClassDecl> = vec![class];
        for name in &descendants {
            if let Some(c) = program.classes.iter().find(|c| &c.name == name) {
                candidates.push(c);
            }
        }

        for method_name in callable_instance_method_names(&class.name, &program.classes) {
            generate_one_class_dispatcher(module, &class.name, &method_name, &candidates, &program.classes);
        }
    }
}

/// Toutes les classes qui étendent (directement ou transitivement)
/// `class_name` — aussi utilisée par `program.rs` pour construire l'ensemble
/// de candidats d'un `is ClassName` réel (voir `IrModule::is_check_candidates`).
pub fn transitive_descendants(class_name: &str, all_classes: &[ClassDecl]) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let mut frontier = vec![class_name.to_string()];
    while let Some(current) = frontier.pop() {
        for c in all_classes {
            if c.extends.as_deref() == Some(current.as_str()) && !result.contains(&c.name) {
                result.push(c.name.clone());
                frontier.push(c.name.clone());
            }
        }
    }
    result
}

/// Noms des méthodes D'INSTANCE appelables sur `class_name` (propres ou
/// héritées en remontant `extends`) — jamais les méthodes statiques (jamais
/// polymorphes, appelées `Class::method()` sans `self`) ni le constructeur.
fn callable_instance_method_names(class_name: &str, all_classes: &[ClassDecl]) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut current = Some(class_name.to_string());
    while let Some(c) = current {
        let Some(decl) = all_classes.iter().find(|d| d.name == c) else { break };
        for member in &decl.members {
            if let ClassMember::Method { decl: m, is_static: false, .. } = member {
                names.insert(m.name.clone());
            }
        }
        current = decl.extends.clone();
    }
    names
}

fn generate_one_class_dispatcher(
    module: &mut IrModule,
    class_name: &str,
    method_name: &str,
    candidates: &[&ClassDecl],
    all_classes: &[ClassDecl],
) {
    // Signature de la méthode : celle du candidat le plus proche de
    // `class_name` (lui-même, s'il déclare la méthode — sinon un ancêtre,
    // possiblement HORS de `candidates` qui ne contient que `class_name` et
    // ses descendants) — tous les candidats partagent la même signature
    // (héritage sans changement de signature, seule sa présence dans les
    // membres varie).
    let Some(sig) = find_method_decl(class_name, method_name, all_classes) else { return };
    let ret_ty = IrType::from_ast(&sig.ret_ty);

    let mut f = IrFunction::new(format!("__dispatch_{}_{}", class_name, method_name), vec![], ret_ty.clone());

    let self_val = f.new_value();
    let mut params = vec![IrParam { name: "self".into(), ty: IrType::Ptr, slot: self_val.clone() }];
    let mut arg_vals = Vec::new();
    for p in &sig.params {
        let v = f.new_value();
        params.push(IrParam { name: p.name.clone(), ty: IrType::from_ast(&p.ty), slot: v.clone() });
        arg_vals.push(v);
    }
    f.params = params;

    let class_id_val = f.new_value();
    f.emit(Inst::GetField {
        dest:   class_id_val.clone(),
        obj:    self_val.clone(),
        field:  "__class_id".into(),
        ty:     IrType::I64,
        offset: -16,
    });

    for candidate in candidates {
        let class_id = module.class_ids.get(&candidate.name).copied().unwrap_or(0);
        let cid_const = f.new_value();
        f.emit(Inst::ConstInt { dest: cid_const.clone(), value: class_id });
        let cmp = f.new_value();
        f.emit(Inst::CmpEq { dest: cmp.clone(), lhs: class_id_val.clone(), rhs: cid_const.clone(), ty: IrType::I64 });

        let call_bb = f.new_block();
        let next_bb = f.new_block();
        f.emit(Inst::Branch { cond: cmp, then_bb: call_bb.clone(), else_bb: next_bb.clone() });

        f.switch_to(&call_bb);
        let callee = format!("{}_{}", candidate.name, method_name);
        let mut call_args = vec![self_val.clone()];
        call_args.extend(arg_vals.clone());
        if ret_ty == IrType::Void {
            f.emit(Inst::Call { dest: None, func: callee, args: call_args, ret_ty: IrType::Void });
            f.emit(Inst::Return { value: None });
        } else {
            let r = f.new_value();
            f.emit(Inst::Call { dest: Some(r.clone()), func: callee, args: call_args, ret_ty: ret_ty.clone() });
            f.emit(Inst::Return { value: Some(r) });
        }

        f.switch_to(&next_bb);
    }

    // Défensif : `class_id` ne correspond à aucun candidat — ne devrait
    // jamais arriver (`self` d'une méthode de cette famille de classes est
    // forcément l'une d'elles).
    if ret_ty == IrType::Void {
        f.emit(Inst::Return { value: None });
    } else {
        let zero = f.new_value();
        f.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
        f.emit(Inst::Return { value: Some(zero) });
    }

    module.add_function(f);
}

/// Trouve la déclaration de `method_name` la plus proche de `class_name` en
/// remontant `extends` dans `all_classes` (PAS seulement les descendants) —
/// utilisée uniquement pour connaître la signature (params/type de retour),
/// identique pour tous les candidats.
fn find_method_decl<'a>(class_name: &str, method_name: &str, all_classes: &'a [ClassDecl]) -> Option<&'a FuncDecl> {
    let mut current = all_classes.iter().find(|c| c.name == class_name);
    while let Some(decl) = current {
        for member in &decl.members {
            if let ClassMember::Method { decl: m, is_static: false, .. } = member {
                if m.name == method_name {
                    return Some(m);
                }
            }
        }
        current = decl.extends.as_ref().and_then(|p| all_classes.iter().find(|c| &c.name == p));
    }
    None
}
