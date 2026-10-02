/// `init()` synthétisé pour porter des initialiseurs de `property` (voir
/// `parsing::parser::property_init`) dans une classe sans constructeur
/// propre dont un ANCÊTRE en a un : sans ce complément, il masquerait le
/// constructeur hérité (paramètres perdus). Il reçoit donc les paramètres du
/// constructeur effectif du parent et commence par `parent::init(...)` — le
/// parent est construit d'abord, puis les initialiseurs de la classe
/// s'appliquent. Tourne une fois tous les imports fusionnés (parent
/// éventuellement importé), avant la table des symboles.

use std::collections::HashMap;
use crate::parsing::ast::{ClassDecl, ClassMember, Expr, Param, Program, Stmt};

pub fn complete_implicit_inits(program: &mut Program) {
    let mut resolved: HashMap<String, Option<Vec<Param>>> = HashMap::new();
    let names: Vec<String> = program.classes.iter().map(|c| c.name.clone()).collect();
    for name in &names {
        ctor_params(&program.classes, name, &mut resolved, 0);
    }
    for class in program.classes.iter_mut().filter(|c| c.implicit_init && c.extends.is_some()) {
        let parent = class.extends.clone().unwrap();
        let Some(Some(params)) = resolved.get(&parent).cloned() else { continue };
        let Some(ClassMember::Constructor { params: own, body, span }) = class.members.iter_mut()
            .find(|m| matches!(m, ClassMember::Constructor { .. }))
        else { continue };
        let args = params.iter().map(|p| Expr::Ident(p.name.clone(), p.span.clone())).collect();
        let call = Expr::StaticCall { class: "<parent>".into(), method: "init".into(), args, span: span.clone() };
        body.stmts.insert(0, Stmt::Expr(call));
        *own = params;
    }
}

/// Paramètres du constructeur effectif de `name` (le sien, implicite
/// compris une fois complété, sinon celui de l'ancêtre le plus proche) ;
/// `None` : aucun constructeur dans la chaîne.
fn ctor_params(classes: &[ClassDecl], name: &str, resolved: &mut HashMap<String, Option<Vec<Param>>>, depth: usize) -> Option<Vec<Param>> {
    if let Some(known) = resolved.get(name) {
        return known.clone();
    }
    let class = classes.iter().find(|c| c.name == name)?;
    let parent = || class.extends.as_deref().filter(|_| depth < 32);
    let result = match class.members.iter().find_map(|m| match m {
        ClassMember::Constructor { params, .. } => Some(params.clone()),
        _ => None,
    }) {
        Some(_) if class.implicit_init => parent()
            .and_then(|p| ctor_params(classes, p, resolved, depth + 1))
            .or(Some(Vec::new())),
        Some(own) => Some(own),
        None => parent().and_then(|p| ctor_params(classes, p, resolved, depth + 1)),
    };
    resolved.insert(name.to_string(), result.clone());
    result
}
