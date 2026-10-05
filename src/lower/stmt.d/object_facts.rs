/// Faits du programme sur la propriété des objets (voir `object_owners`),
/// calculés par point fixe « optimiste puis décroissant » : un fait reste
/// vrai tant qu'aucun corps ne le contredit sous les hypothèses courantes.
/// - `fresh_returns` : fonctions/méthodes qui ne retournent que des objets neufs ;
/// - `fresh_containers` : … qui ne retournent que des conteneurs neufs ;
/// - `preserving` : paramètres conteneurs dont l'appelé ne garde rien et
///   n'insère que des objets neufs ;
/// - `owning_fields` : champs conteneurs d'objets (par nom, tous les accès
///   du programme pris en compte) qui possèdent leurs objets — libérés et
///   clonés par `__free_`/`__clone_<Classe>`.
///
/// Un champ alimenté par un paramètre (`self.items = items`, constructeur
/// de `struct` compris) n'est propriétaire que si TOUS les sites d'appel
/// passent un conteneur neuf ; un appel `obj.m(...)`/`parent::m(...)` est
/// rapproché par nom de méthode, et une fonction référencée comme valeur
/// (appelable indirectement) ne reçoit jamais de transfert.
use std::collections::{HashMap, HashSet};

use crate::parsing::ast::{Block, Param, Type};
use super::object_owners::{body_facts, is_object_container, FreshCtx};

/// (clé, classe, paramètres, corps, type de retour).
pub type Callable<'a> = (String, Option<&'a str>, &'a [Param], &'a Block, Option<Type>);

#[derive(Default)]
pub struct ObjectFacts {
    pub fresh_returns:    HashSet<String>,
    pub fresh_containers: HashSet<String>,
    pub preserving:       HashMap<String, Vec<bool>>,
    pub owning_fields:    HashSet<String>,
}

pub fn compute(
    callables: &[Callable],
    param_types: &HashMap<String, Vec<Type>>,
    param_keeps: &HashMap<String, Vec<bool>>,
    field_types: &HashMap<String, Vec<(String, Type)>>,
) -> ObjectFacts {
    let mut facts = ObjectFacts {
        fresh_returns: callables.iter().filter(|c| matches!(c.4, Some(Type::Named(_)))).map(|c| c.0.clone()).collect(),
        fresh_containers: callables.iter().filter(|c| c.4.as_ref().is_some_and(is_object_container)).map(|c| c.0.clone()).collect(),
        preserving: callables.iter().map(|c| (c.0.clone(), c.2.iter().map(|p| is_object_container(&p.ty)).collect())).collect(),
        owning_fields: field_types.values().flatten().filter(|(_, t)| is_object_container(t)).map(|(f, _)| f.clone()).collect(),
    };
    // Phase 1 : fonctions et paramètres, champs supposés propriétaires (un
    // paramètre supposé « préservant » ferait passer un transfert vers un
    // champ pour un simple prêt, et retirerait le champ trop tôt).
    let fields = facts.owning_fields.clone();
    loop {
        let mut next = step(&facts, callables, param_types, param_keeps);
        next.owning_fields = fields.clone();
        let stable = next.fresh_returns == facts.fresh_returns && next.fresh_containers == facts.fresh_containers
            && next.preserving == facts.preserving;
        facts = next;
        if stable { break; }
    }
    // Phase 2 : champs, paramètres stabilisés.
    loop {
        let next = step(&facts, callables, param_types, param_keeps);
        let stable = next.owning_fields == facts.owning_fields;
        facts.owning_fields = next.owning_fields;
        if stable { return facts; }
    }
}

fn step(facts: &ObjectFacts, callables: &[Callable], param_types: &HashMap<String, Vec<Type>>, param_keeps: &HashMap<String, Vec<bool>>) -> ObjectFacts {
    let mut next = ObjectFacts { owning_fields: facts.owning_fields.clone(), ..ObjectFacts::default() };
    let mut field_from_param: Vec<(String, String, usize)> = Vec::new();
    let mut calls: Vec<(String, Vec<bool>)> = Vec::new();
    let mut value_refs: HashSet<String> = HashSet::new();
    for (key, class, params, body, _) in callables {
        let ctx = FreshCtx {
            fresh_returns: &facts.fresh_returns, fresh_containers: &facts.fresh_containers,
            preserving: &facts.preserving, param_types, current_class: *class,
        };
        let bf = body_facts(body, params, &ctx, &facts.owning_fields);
        if facts.fresh_returns.contains(key) && bf.returns_fresh_object { next.fresh_returns.insert(key.clone()); }
        if facts.fresh_containers.contains(key) && bf.returns_fresh_container { next.fresh_containers.insert(key.clone()); }
        let keeps = param_keeps.get(key);
        let old = facts.preserving.get(key);
        let preserving = bf.preserving_params.iter().enumerate()
            .map(|(i, p)| p.unwrap_or(false)
                && old.and_then(|o| o.get(i)).copied().unwrap_or(false)
                && !keeps.and_then(|k| k.get(i)).copied().unwrap_or(false))
            .collect();
        next.preserving.insert(key.clone(), preserving);
        next.owning_fields.retain(|f| !bf.disqualified_fields.contains(f));
        field_from_param.extend(bf.field_from_param.into_iter().map(|(f, i)| (f, key.clone(), i)));
        calls.extend(bf.calls);
        value_refs.extend(bf.value_refs);
    }
    for (field, key, i) in field_from_param {
        let method = callables.iter().find(|c| c.0 == key)
            .map(|c| c.1.map_or(key.clone(), |cls| key.strip_prefix(&format!("{}_", cls)).unwrap_or(&key).to_string()))
            .unwrap_or_else(|| key.clone());
        let by_name = format!(".{}", method);
        let referenced = value_refs.contains(&key) || value_refs.contains(&by_name);
        let all_fresh = calls.iter()
            .filter(|(callee, _)| *callee == key || *callee == by_name)
            .all(|(_, args)| args.get(i).copied().unwrap_or(false));
        if referenced || !all_fresh {
            next.owning_fields.remove(&field);
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::compute;
    use crate::parsing::ast::ClassMember;
    use crate::parsing::{lexer::Lexer, parser::Parser};
    use std::collections::HashMap;

    fn facts(src: &str) -> super::ObjectFacts {
        let program = Parser::new(Lexer::new(src).tokenize().unwrap()).parse_program().unwrap();
        let mut callables: Vec<super::Callable> = program.functions.iter()
            .map(|f| (f.name.clone(), None, f.params.as_slice(), &f.body, Some(f.ret_ty.clone())))
            .collect();
        let mut field_types = HashMap::new();
        let mut param_types = HashMap::new();
        for c in &program.classes {
            let mut fields = Vec::new();
            for m in &c.members {
                match m {
                    ClassMember::Field { name, ty, .. } => fields.push((name.clone(), ty.clone())),
                    ClassMember::Method { decl, .. } => callables.push((format!("{}_{}", c.name, decl.name), Some(c.name.as_str()), decl.params.as_slice(), &decl.body, Some(decl.ret_ty.clone()))),
                    ClassMember::Constructor { params, body, .. } => callables.push((format!("{}_init", c.name), Some(c.name.as_str()), params.as_slice(), body, None)),
                    _ => {}
                }
            }
            field_types.insert(c.name.clone(), fields);
        }
        for (k, _, params, _, _) in &callables { param_types.insert(k.clone(), params.iter().map(|p| p.ty.clone()).collect()); }
        compute(&callables, &param_types, &HashMap::new(), &field_types)
    }

    const ITEM: &str = "class Item {\n    init() { }\n}\n";

    #[test]
    fn containers_returned_and_transferred_to_fields() {
        let src = format!("{}class Dto {{\n    public property items:array<Item>\n    init(items:array<Item>) {{\n        self.items = items\n    }}\n}}\nfunction all(): array<Item> {{\n    var xs:array<Item> = []\n    xs.push(use Item())\n    return xs\n}}\nfunction main(): int {{\n    var d:Dto = use Dto(all())\n    var ys:array<Item> = [use Item()]\n    var e:Dto = use Dto(ys)\n    return 0\n}}\n", ITEM);
        let f = facts(&src);
        assert!(f.fresh_containers.contains("all"));
        assert!(f.owning_fields.contains("items"));
    }

    #[test]
    fn shared_transfer_or_extraction_disqualifies_the_field() {
        let shared = format!("{}class Dto {{\n    public property items:array<Item>\n    init(items:array<Item>) {{\n        self.items = items\n    }}\n}}\nfunction main(): int {{\n    var ys:array<Item> = [use Item()]\n    var d:Dto = use Dto(ys)\n    var e:Dto = use Dto(ys)\n    return 0\n}}\n", ITEM);
        assert!(!facts(&shared).owning_fields.contains("items"));
        let extracted = format!("{}class Dto {{\n    public property items:array<Item>\n    init() {{\n        self.items = []\n    }}\n    public method first(): Item {{\n        return self.items[0]\n    }}\n}}\nfunction main(): int {{ return 0 }}\n", ITEM);
        assert!(!facts(&extracted).owning_fields.contains("items"));
    }

    #[test]
    fn preserving_parameters() {
        let src = format!("{}function fill(xs:array<Item>): void {{\n    xs.push(use Item())\n}}\nfunction leak(xs:array<Item>): Item {{\n    return xs[0]\n}}\nfunction main(): int {{ return 0 }}\n", ITEM);
        let f = facts(&src);
        assert_eq!(f.preserving.get("fill"), Some(&vec![true]));
        assert_eq!(f.preserving.get("leak"), Some(&vec![false]));
    }
}
