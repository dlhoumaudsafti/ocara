/// `struct` : vérifications de déclaration et constructeur hérité — voir
/// docs/roadmap.d/langage-struct-value-type.md et `docs/EBNF.md` §16.7.
///
/// Tourne une fois tous les imports fusionnés (un struct parent peut vivre
/// dans un autre fichier), avant la table des symboles :
///   - E51 : champ `private` (un struct n'a pas d'invariant à protéger) ;
///   - E52 : `extends` entre struct et class, dans un sens ou dans l'autre ;
///   - E53 : champ d'un struct parent redéclaré par un struct enfant ;
///   - le constructeur généré par le parser (champs propres) reçoit en tête
///     les paramètres et affectations de toute la chaîne de parents.
///
/// Un parent inexistant est laissé au diagnostic existant (E27, `main.rs`).

use std::collections::HashMap;
use crate::parsing::ast::{ClassDecl, ClassMember, Param, Program, Visibility};
use crate::parsing::parser::struct_decl::assign_self_field;
use crate::parsing::token::Span;

pub type StructError = (Span, String);

pub fn expand_structs(program: &mut Program) -> Result<(), StructError> {
    check_declarations(&program.classes)?;

    let by_name: HashMap<&str, &ClassDecl> = program.classes.iter().map(|c| (c.name.as_str(), c)).collect();
    let mut inherited: HashMap<String, Vec<Param>> = HashMap::new();
    for class in program.classes.iter().filter(|c| c.is_struct && c.extends.is_some()) {
        inherited.insert(class.name.clone(), ancestor_params(class, &by_name)?);
    }

    for class in program.classes.iter_mut() {
        let Some(parent_params) = inherited.remove(&class.name) else { continue };
        let Some(ClassMember::Constructor { params, body, .. }) = class.members.iter_mut()
            .find(|m| matches!(m, ClassMember::Constructor { .. }))
        else { continue };
        let parent_assigns = parent_params.iter().map(|p| assign_self_field(&p.name, &p.span));
        body.stmts.splice(0..0, parent_assigns);
        params.splice(0..0, parent_params);
    }
    Ok(())
}

fn check_declarations(classes: &[ClassDecl]) -> Result<(), StructError> {
    let is_struct: HashMap<&str, bool> = classes.iter().map(|c| (c.name.as_str(), c.is_struct)).collect();
    for class in classes {
        if class.is_struct {
            if let Some(span) = class.members.iter().find_map(|m| match m {
                ClassMember::Field { vis: Visibility::Private, span, .. }
                | ClassMember::Const { vis: Visibility::Private, span, .. } => Some(span),
                _ => None,
            }) {
                return Err((span.clone(), format!(
                    "'private' is not allowed in struct '{}' — a struct is a transparent data aggregate with no invariant to protect; use 'protected' (visible to extending structs) or a class", class.name)));
            }
        }
        let Some(parent) = &class.extends else { continue };
        match (class.is_struct, is_struct.get(parent.as_str())) {
            (true, Some(false)) => return Err((class.span.clone(), format!(
                "struct '{}' cannot extend class '{}' — a struct can only extend another struct", class.name, parent))),
            (false, Some(true)) => return Err((class.span.clone(), format!(
                "class '{}' cannot extend struct '{}' — a struct can only be extended by another struct", class.name, parent))),
            _ => {}
        }
    }
    Ok(())
}

/// Paramètres du constructeur de toute la chaîne de parents de `class`,
/// du plus lointain au plus proche (ordre de déclaration des champs).
fn ancestor_params(class: &ClassDecl, by_name: &HashMap<&str, &ClassDecl>) -> Result<Vec<Param>, StructError> {
    let mut chain: Vec<&ClassDecl> = Vec::new();
    let mut current = class.extends.as_deref();
    while let Some(name) = current {
        let Some(parent) = by_name.get(name) else { break };
        if chain.iter().any(|c| c.name == parent.name) || parent.name == class.name { break; }
        chain.push(parent);
        current = parent.extends.as_deref();
    }

    let mut params: Vec<Param> = Vec::new();
    for parent in chain.iter().rev() {
        params.extend(own_params(parent).iter().cloned());
    }
    for own in own_params(class) {
        if let Some(parent_field) = params.iter().find(|p| p.name == own.name) {
            return Err((own.span.clone(), format!(
                "field '{}' of struct '{}' is already declared by a parent struct (line {}) — a struct cannot redeclare an inherited field",
                own.name, class.name, parent_field.span.line)));
        }
    }
    Ok(params)
}

/// Paramètres du constructeur généré par le parser : champs PROPRES du struct.
fn own_params(class: &ClassDecl) -> &[Param] {
    class.members.iter()
        .find_map(|m| match m {
            ClassMember::Constructor { params, .. } => Some(params.as_slice()),
            _ => None,
        })
        .unwrap_or(&[])
}
