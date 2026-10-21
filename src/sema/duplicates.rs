/// Déclarations en double (E03) : deux fonctions, deux types (classe,
/// interface, generic, enum, module) ou deux membres d'une même classe de
/// même nom. Sans ce contrôle, la première gagnait en silence — ou le
/// codegen échouait (`DuplicateDefinition`).

use std::collections::HashSet;

use crate::parsing::ast::{ClassMember, Program};
use crate::parsing::token::Span;
use crate::sema::error::SemaError;

pub fn check_program(program: &Program) -> Vec<SemaError> {
    let mut errors = Vec::new();
    let mut functions = HashSet::new();
    for f in &program.functions {
        report(&mut errors, &mut functions, &f.name, &f.span);
    }
    let mut types = HashSet::new();
    let declared = program.classes.iter().map(|c| (&c.name, &c.span))
        .chain(program.interfaces.iter().map(|i| (&i.name, &i.span)))
        .chain(program.generics.iter().map(|g| (&g.name, &g.span)))
        .chain(program.enums.iter().map(|e| (&e.name, &e.span)))
        .chain(program.modules.iter().map(|m| (&m.name, &m.span)));
    for (name, span) in declared {
        report(&mut errors, &mut types, name, span);
    }
    let bodies = program.classes.iter().map(|c| (&c.name, &c.members))
        .chain(program.generics.iter().map(|g| (&g.name, &g.members)))
        .chain(program.modules.iter().map(|m| (&m.name, &m.members)));
    for (owner, members) in bodies {
        check_members(&mut errors, owner, members);
    }
    errors
}

fn check_members(errors: &mut Vec<SemaError>, owner: &str, members: &[ClassMember]) {
    let mut methods = HashSet::new();
    let mut fields = HashSet::new();
    let mut constructors = 0;
    for member in members {
        match member {
            ClassMember::Method { decl, .. } => report(errors, &mut methods, &format!("{}::{}", owner, decl.name), &decl.span),
            ClassMember::Field { name, span, .. } | ClassMember::Const { name, span, .. } =>
                report(errors, &mut fields, &format!("{}.{}", owner, name), span),
            ClassMember::Constructor { span, .. } => {
                constructors += 1;
                if constructors > 1 {
                    errors.push(SemaError::DuplicateSymbol { name: format!("{}::init", owner), span: span.clone() });
                }
            }
        }
    }
}

fn report(errors: &mut Vec<SemaError>, seen: &mut HashSet<String>, name: &str, span: &Span) {
    if !seen.insert(name.to_string()) {
        errors.push(SemaError::DuplicateSymbol { name: name.to_string(), span: span.clone() });
    }
}
