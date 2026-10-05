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

/// `"Classe.champ"` → `"ClasseDéclarante.champ"` : un champ hérité désigne
/// le stockage déclaré par l'ancêtre le plus haut qui le possède.
pub fn field_declarations(field_types: &HashMap<String, Vec<(String, Type)>>, parents: &HashMap<String, String>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (class, fields) in field_types {
        for (field, _) in fields {
            let mut decl = class.clone();
            while let Some(parent) = parents.get(&decl) {
                let has = field_types.get(parent).is_some_and(|fs| fs.iter().any(|(f, _)| f == field));
                if !has { break; }
                decl = parent.clone();
            }
            out.insert(format!("{}.{}", class, field), format!("{}.{}", decl, field));
        }
    }
    out
}

/// Contexte constant du calcul.
pub struct Program<'a> {
    pub callables:   &'a [Callable<'a>],
    pub param_types: &'a HashMap<String, Vec<Type>>,
    pub param_keeps: &'a HashMap<String, Vec<bool>>,
    pub field_types: &'a HashMap<String, Vec<(String, Type)>>,
    pub field_decl:  &'a HashMap<String, String>,
    pub parents:     &'a HashMap<String, String>,
}

pub fn compute(prog: &Program) -> ObjectFacts {
    let (callables, field_types) = (prog.callables, prog.field_types);
    let mut facts = ObjectFacts {
        fresh_returns: callables.iter().filter(|c| matches!(c.4, Some(Type::Named(_)))).map(|c| c.0.clone()).collect(),
        fresh_containers: callables.iter().filter(|c| c.4.as_ref().is_some_and(is_object_container)).map(|c| c.0.clone()).collect(),
        preserving: callables.iter().map(|c| (c.0.clone(), c.2.iter().map(|p| is_object_container(&p.ty)).collect())).collect(),
        owning_fields: field_types.iter()
            .flat_map(|(c, fs)| fs.iter().filter(|(_, t)| is_object_container(t)).map(move |(f, _)| format!("{}.{}", c, f)))
            .filter_map(|k| prog.field_decl.get(&k).cloned())
            .collect(),
    };
    // Phase 1 : fonctions et paramètres, champs supposés propriétaires (un
    // paramètre supposé « préservant » ferait passer un transfert vers un
    // champ pour un simple prêt, et retirerait le champ trop tôt).
    let fields = facts.owning_fields.clone();
    loop {
        let mut next = step(&facts, prog);
        next.owning_fields = fields.clone();
        let stable = next.fresh_returns == facts.fresh_returns && next.fresh_containers == facts.fresh_containers
            && next.preserving == facts.preserving;
        facts = next;
        if stable { break; }
    }
    // Phase 2 : champs, paramètres stabilisés.
    loop {
        let next = step(&facts, prog);
        let stable = next.owning_fields == facts.owning_fields;
        facts.owning_fields = next.owning_fields;
        if stable { return facts; }
    }
}

fn step(facts: &ObjectFacts, prog: &Program) -> ObjectFacts {
    let (callables, param_types, param_keeps) = (prog.callables, prog.param_types, prog.param_keeps);
    let mut next = ObjectFacts { owning_fields: facts.owning_fields.clone(), ..ObjectFacts::default() };
    let mut field_from_param: Vec<(String, String, usize)> = Vec::new();
    let mut calls: Vec<(String, Vec<bool>)> = Vec::new();
    let mut value_refs: HashSet<String> = HashSet::new();
    for (key, class, params, body, _) in callables {
        let ctx = FreshCtx {
            fresh_returns: &facts.fresh_returns, fresh_containers: &facts.fresh_containers,
            preserving: &facts.preserving, param_types, field_types: prog.field_types,
            field_decl: prog.field_decl, current_class: *class,
            parent_class: class.and_then(|c| prog.parents.get(c)).map(String::as_str),
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
        // Un site rapproché par nom qui passe plus d'arguments que la méthode
        // n'a de paramètres ne peut pas l'appeler.
        let arity = callables.iter().find(|c| c.0 == key).map_or(usize::MAX, |c| c.2.len());
        let all_fresh = calls.iter()
            .filter(|(callee, args)| *callee == key || (*callee == by_name && args.len() <= arity))
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
        let field_decl = super::field_declarations(&field_types, &HashMap::new());
        let keeps = HashMap::new();
        compute(&super::Program { callables: &callables, param_types: &param_types, param_keeps: &keeps, field_types: &field_types, field_decl: &field_decl, parents: &HashMap::new() })
    }

    const ITEM: &str = "class Item {\n    init() { }\n}\n";

    #[test]
    fn containers_returned_and_transferred_to_fields() {
        let src = format!("{}class Dto {{\n    public property items:array<Item>\n    init(items:array<Item>) {{\n        self.items = items\n    }}\n}}\nfunction all(): array<Item> {{\n    var xs:array<Item> = []\n    xs.push(use Item())\n    return xs\n}}\nfunction main(): int {{\n    var d:Dto = use Dto(all())\n    var ys:array<Item> = [use Item()]\n    var e:Dto = use Dto(ys)\n    return 0\n}}\n", ITEM);
        let f = facts(&src);
        assert!(f.fresh_containers.contains("all"));
        assert!(f.owning_fields.contains("Dto.items"));
    }

    #[test]
    fn shared_transfer_or_extraction_disqualifies_the_field() {
        let shared = format!("{}class Dto {{\n    public property items:array<Item>\n    init(items:array<Item>) {{\n        self.items = items\n    }}\n}}\nfunction main(): int {{\n    var ys:array<Item> = [use Item()]\n    var d:Dto = use Dto(ys)\n    var e:Dto = use Dto(ys)\n    return 0\n}}\n", ITEM);
        assert!(!facts(&shared).owning_fields.contains("Dto.items"));
        let extracted = format!("{}class Dto {{\n    public property items:array<Item>\n    init() {{\n        self.items = []\n    }}\n    public method first(): Item {{\n        return self.items[0]\n    }}\n}}\nfunction main(): int {{ return 0 }}\n", ITEM);
        assert!(!facts(&extracted).owning_fields.contains("Dto.items"));
    }

    const BAG: &str = "class Bag {\n    public property items:array<Item>\n    init(items:array<Item>) {\n        self.items = items\n    }\n}\n";

    fn owns(main_body: &str) -> bool {
        let src = format!("{}{}function main(): int {{\n{}\n    return 0\n}}\n", ITEM, BAG, main_body);
        facts(&src).owning_fields.contains("Bag.items")
    }

    #[test]
    fn moves_inside_loops_or_followed_by_reads_are_refused() {
        assert!(owns("    var ys:array<Item> = [use Item()]\n    var n:int = ys.len()\n    var b:Bag = use Bag(ys)"));
        assert!(!owns("    var ys:array<Item> = [use Item()]\n    var i:int = 0\n    while i smaller 2 {\n        var b:Bag = use Bag(ys)\n        i = i + 1\n    }"));
        assert!(!owns("    var ys:array<Item> = [use Item()]\n    var b:Bag = use Bag(ys)\n    var n:int = ys.len()"));
    }

    #[test]
    fn element_kept_after_move_to_self_disqualifies_the_field() {
        let src = format!("{}class Keeper {{\n    public property items:array<Item>\n    init() {{\n        self.items = []\n    }}\n    public method adopt(): Item {{\n        var xs:array<Item> = [use Item()]\n        self.items = xs\n        return xs[0]\n    }}\n}}\nfunction main(): int {{ return 0 }}\n", ITEM);
        assert!(!facts(&src).owning_fields.contains("Keeper.items"));
    }

    #[test]
    fn fields_are_told_apart_by_class() {
        let src = format!("{}{}class Other {{\n    public property items:array<Item>\n    init(items:array<Item>) {{\n        self.items = items\n    }}\n}}\nfunction main(): int {{\n    var b:Bag = use Bag([use Item()])\n    var ys:array<Item> = [use Item()]\n    var o:Other = use Other(ys)\n    var p:Other = use Other(ys)\n    return 0\n}}\n", ITEM, BAG);
        let f = facts(&src);
        assert!(f.owning_fields.contains("Bag.items"));
        assert!(!f.owning_fields.contains("Other.items"));
    }

    #[test]
    fn resolved_async_containers_are_fresh() {
        let src = format!("{}{}function load(): array<Item> {{\n    var xs:array<Item> = []\n    xs.push(use Item())\n    return xs\n}}\nfunction main(): int {{\n    consumed t:Resolvable<array<Item>> = load()\n    const xs:array<Item> = resolve t\n    var b:Bag = use Bag(xs)\n    return 0\n}}\n", ITEM, BAG);
        assert!(facts(&src).owning_fields.contains("Bag.items"));
    }

    #[test]
    fn preserving_parameters() {
        let src = format!("{}function fill(xs:array<Item>): void {{\n    xs.push(use Item())\n}}\nfunction leak(xs:array<Item>): Item {{\n    return xs[0]\n}}\nfunction main(): int {{ return 0 }}\n", ITEM);
        let f = facts(&src);
        assert_eq!(f.preserving.get("fill"), Some(&vec![true]));
        assert_eq!(f.preserving.get("leak"), Some(&vec![false]));
    }
}
