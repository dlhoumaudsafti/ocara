//! Déclaration désignée par une référence de la sema, retrouvée dans l'AST
//! fusionné : position et signature affichable.

use crate::parsing::ast::{ClassMember, FuncDecl, Param, Program, Type};
use crate::parsing::token::Span;
use crate::sema::index::Target;

pub struct Decl {
    pub name: String,
    pub span: Span,
    pub signature: String,
}

pub fn find(program: &Program, target: &Target) -> Option<Decl> {
    match target {
        Target::Local { .. } => None,
        Target::Const(name) => program.consts.iter().find(|c| &c.name == name)
            .map(|c| decl(name, &c.span, format!("const {}:{}", name, display_type(&c.ty)))),
        Target::Function(name) => program.functions.iter().find(|f| &f.name == name)
            .map(|f| decl(name, &f.span, function_signature("function ", f))),
        Target::Class(name) => find_type(program, name),
        Target::Method { class, name } => find_member(program, class, |owner, m| match m {
            ClassMember::Method { decl: f, is_static, .. } if &f.name == name => {
                let sep = if *is_static { "::" } else { "." };
                Some((f.span.clone(), function_signature(&format!("{}{}", owner, sep), f)))
            }
            ClassMember::Constructor { params, span, .. } if name == "init" => {
                Some((span.clone(), format!("use {}({})", owner, params_list(params))))
            }
            _ => None,
        }).map(|(span, sig)| decl(name, &span, sig)),
        Target::Field { class, name } => find_member(program, class, |owner, m| match m {
            ClassMember::Field { name: n, ty, span, .. } if n == name => Some((span.clone(), format!("{}.{}:{}", owner, name, display_type(ty)))),
            _ => None,
        }).map(|(span, sig)| decl(name, &span, sig)),
        Target::Param { callee, name } => {
            let params = match callee.split_once("::") {
                Some((class, method)) => find_member(program, class, |_, m| match m {
                    ClassMember::Method { decl: f, .. } if f.name == method => Some(f.params.clone()),
                    ClassMember::Constructor { params, .. } if method == "init" => Some(params.clone()),
                    _ => None,
                })?,
                None => program.functions.iter().find(|f| &f.name == callee)?.params.clone(),
            };
            let p = params.iter().find(|p| &p.name == name)?;
            Some(decl(name, &p.span, format!("(paramètre de {}) {}", callee, params_list(std::slice::from_ref(p)))))
        }
        Target::ClassConst { class, name } => find_member(program, class, |owner, m| match m {
            ClassMember::Const { name: n, ty, span, .. } if n == name => Some((span.clone(), format!("{}::{}:{}", owner, name, display_type(ty)))),
            _ => None,
        }).map(|(span, sig)| decl(name, &span, sig)),
    }
}

fn decl(name: &str, span: &Span, signature: String) -> Decl {
    Decl { name: name.to_string(), span: span.clone(), signature }
}

fn find_type(program: &Program, name: &str) -> Option<Decl> {
    if let Some(c) = program.classes.iter().find(|c| c.name == name) {
        let mut header = format!("{} {}", if c.is_struct { "struct" } else { "class" }, name);
        if let Some(parent) = &c.extends { header.push_str(&format!(" extends {}", parent)); }
        if !c.implements.is_empty() { header.push_str(&format!(" implements {}", c.implements.join(", "))); }
        if let Some(params) = c.members.iter().find_map(|m| match m { ClassMember::Constructor { params, .. } => Some(params), _ => None }) {
            header.push_str(&format!("\nuse {}({})", name, params_list(params)));
        }
        return Some(decl(name, &c.span, header));
    }
    if let Some(g) = program.generics.iter().find(|g| g.name == name) {
        let params: Vec<&str> = g.type_params.iter().map(|p| p.name.as_str()).collect();
        return Some(decl(name, &g.span, format!("generic {}<{}>", name, params.join(", "))));
    }
    if let Some(i) = program.interfaces.iter().find(|i| i.name == name) {
        return Some(decl(name, &i.span, format!("interface {}", name)));
    }
    if let Some(e) = program.enums.iter().find(|e| e.name == name) {
        return Some(decl(name, &e.span, format!("enum {}", name)));
    }
    program.modules.iter().find(|m| m.name == name).map(|m| decl(name, &m.span, format!("module {}", name)))
}

/// Membre de `class` ou d'un de ses parents (classe, generic ou module) ;
/// `pick` reçoit la classe qui le déclare.
fn find_member<T>(program: &Program, class: &str, pick: impl Fn(&str, &ClassMember) -> Option<T>) -> Option<T> {
    let mut current = Some(class.to_string());
    let mut seen = 0;
    while let Some(name) = current.take() {
        seen += 1;
        if seen > 64 { return None; }
        let (members, parent) = if let Some(c) = program.classes.iter().find(|c| c.name == name) {
            (&c.members, c.extends.clone())
        } else if let Some(g) = program.generics.iter().find(|g| g.name == name) {
            (&g.members, g.extends.clone())
        } else if let Some(m) = program.modules.iter().find(|m| m.name == name) {
            (&m.members, None)
        } else {
            return None;
        };
        if let Some(found) = members.iter().find_map(|m| pick(&name, m)) {
            return Some(found);
        }
        current = parent;
    }
    None
}

fn function_signature(prefix: &str, f: &FuncDecl) -> String {
    let ret = if matches!(f.ret_ty, Type::Void) { String::from(": void") } else { format!(": {}", display_type(&f.ret_ty)) };
    format!("{}{}{}({}){}", if f.is_async { "async " } else { "" }, prefix, f.name, params_list(&f.params), ret)
}

fn params_list(params: &[Param]) -> String {
    params.iter().map(|p| {
        let variadic = if p.is_variadic { "variadic " } else { "" };
        let default = if p.default_value.is_some() { " = …" } else { "" };
        format!("{}{}:{}{}", variadic, p.name, display_type(&p.ty), default)
    }).collect::<Vec<_>>().join(", ")
}

/// Type tel qu'on l'écrit en Ocara (`array<T>`, `map<K, V>`).
pub fn display_type(ty: &Type) -> String {
    match ty {
        Type::Array(inner) => format!("array<{}>", display_type(inner)),
        Type::Map(k, v) => format!("map<{}, {}>", display_type(k), display_type(v)),
        Type::Message(inner) => format!("message<{}>", display_type(inner)),
        Type::Resolvable(inner) => format!("Resolvable<{}>", display_type(inner)),
        Type::Generic { name, args } => format!("{}<{}>", name, args.iter().map(display_type).collect::<Vec<_>>().join(", ")),
        Type::Union(variants) => variants.iter().map(display_type).collect::<Vec<_>>().join("|"),
        Type::Function { ret_ty, param_tys } => format!("Function<{}({})>", display_type(ret_ty), param_tys.iter().map(display_type).collect::<Vec<_>>().join(", ")),
        other => crate::sema::typecheck::type_name(other),
    }
}
