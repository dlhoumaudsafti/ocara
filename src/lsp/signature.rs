//! Aide à la signature et noms des arguments nommés : cible de l'appel en
//! cours de frappe, résolue par la sema sur le texte patché.

use std::path::Path;

use lsp_types::{
    CompletionItem, CompletionItemKind, Documentation, MarkupContent, MarkupKind, ParameterInformation,
    ParameterLabel, Position, SignatureHelp, SignatureInformation,
};

use super::builtin_docs;
use super::callsite::{call_site, patched_text, CallSite};
use super::decls::{display_type, find_member, param_label};
use super::position::name_range;
use super::workspace::Workspace;
use crate::core::analysis::Analysis;
use crate::parsing::ast::{ClassMember, Param, Program};
use crate::sema::index::Target;

pub struct ParamInfo {
    pub name: String,
    pub label: String,
    pub variadic: bool,
    pub optional: bool,
}

pub struct CallInfo {
    /// `f`, `Classe.m`, `Classe::m`, `use Classe`.
    pub owner: String,
    pub params: Vec<ParamInfo>,
    pub returns: Option<String>,
}

pub fn signature_help(ws: &Workspace, path: &Path, pos: &Position) -> Option<SignatureHelp> {
    let text = ws.text(path)?;
    let site = call_site(&text, pos)?;
    let patched = patched_text(&text, pos);
    let analysis = ws.analyze_text(path, patched.clone());
    let call = resolve_call(&analysis, &patched, &site)?;

    let labels: Vec<&str> = call.params.iter().map(|p| p.label.as_str()).collect();
    let returns = call.returns.as_ref().map(|r| format!(": {}", r)).unwrap_or_default();
    let label = format!("{}({}){}", call.owner, labels.join(", "), returns);
    let mut cursor = call.owner.chars().count() as u32 + 1;
    let parameters = labels.iter().map(|l| {
        let len = l.chars().count() as u32;
        let info = ParameterInformation { label: ParameterLabel::LabelOffsets([cursor, cursor + len]), documentation: None };
        cursor += len + 2;
        info
    }).collect();
    let active = active_parameter(&call, &site);
    Some(SignatureHelp {
        signatures: vec![SignatureInformation { label, documentation: None, parameters: Some(parameters), active_parameter: None }],
        active_signature: Some(0),
        active_parameter: active.map(|a| a as u32),
    })
}

/// Paramètre actif : par nom pour un argument nommé, sinon par position
/// (le variadique absorbe les arguments suivants).
fn active_parameter(call: &CallInfo, site: &CallSite) -> Option<usize> {
    let by_name = |name: &str| call.params.iter().position(|p| p.name == name);
    if let Some(name) = &site.current_name {
        return by_name(name);
    }
    if !site.used_names.is_empty() {
        return call.params.iter().position(|p| !p.variadic && !site.used_names.contains(&p.name));
    }
    match call.params.iter().position(|p| p.variadic) {
        Some(v) => Some(site.arg_index.min(v)),
        None => Some(site.arg_index),
    }
}

/// Noms des paramètres pas encore fournis, dans un appel 100 % nommé (un
/// argument positionnel interdit ensuite les arguments nommés).
pub fn named_argument_items(analysis: &Analysis, patched: &str, site: &CallSite) -> Vec<CompletionItem> {
    if site.positional_count > 0 || site.current_name.is_some() {
        return Vec::new();
    }
    let Some(call) = resolve_call(analysis, patched, site) else { return Vec::new() };
    call.params.iter()
        .filter(|p| !p.variadic && !site.used_names.contains(&p.name))
        .enumerate()
        .map(|(i, p)| CompletionItem {
            label: format!("{}:", p.name),
            kind: Some(CompletionItemKind::PROPERTY),
            detail: Some(p.label.clone()),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("Argument nommé de `{}`{}", call.owner, if p.optional { " — optionnel" } else { "" }),
            })),
            insert_text: Some(format!("{}: ", p.name)),
            filter_text: Some(p.name.clone()),
            sort_text: Some(format!("0{:03}", i)),
            ..Default::default()
        })
        .collect()
}

pub fn resolve_call(analysis: &Analysis, text: &str, site: &CallSite) -> Option<CallInfo> {
    let program = &analysis.checked.as_ref()?.program;
    let reference = analysis.checked.as_ref()?.references.iter()
        .find(|r| r.name == site.callee && name_range(text, &r.span, &r.name).start == site.callee_pos)?;
    match &reference.target {
        Target::Function(name) => {
            let f = program.functions.iter().find(|f| &f.name == name)?;
            Some(user_call(name.clone(), &f.params, Some(display_type(&f.ret_ty))))
        }
        Target::Class(name) => {
            let params = constructor_params(program, name)?;
            Some(user_call(format!("use {}", name), &params, None))
        }
        Target::Method { class, name } => method_call(program, class, name, site.on_instance),
        _ => None,
    }
}

fn method_call(program: &Program, class: &str, name: &str, on_instance: bool) -> Option<CallInfo> {
    let user = find_member(program, class, |owner, m| match m {
        ClassMember::Method { decl, is_static, .. } if decl.name == name => Some((owner.to_string(), *is_static, decl.params.clone(), decl.ret_ty.clone())),
        _ => None,
    });
    if let Some((owner, is_static, params, ret)) = user {
        let sep = if is_static { "::" } else { "." };
        return Some(user_call(format!("{}{}{}", owner, sep, name), &params, Some(display_type(&ret))));
    }
    let mut current = Some(class.to_string());
    while let Some(owner) = current.take() {
        if let Some(m) = builtin_docs::method(&owner, name) {
            // Sucre d'instance (`s.trim()` ≡ `String::trim(s)`) : receveur implicite.
            let skip = usize::from(on_instance && m.is_static);
            let params = m.params.iter().skip(skip).map(|(n, t)| ParamInfo {
                name: n.clone(), label: format!("{}:{}", n, t), variadic: t.starts_with("variadic"), optional: false,
            }).collect();
            let sep = if on_instance { "." } else { "::" };
            return Some(CallInfo { owner: format!("{}{}{}", owner, sep, name), params, returns: Some(m.returns.clone()) });
        }
        current = program.classes.iter().find(|c| c.name == owner).and_then(|c| c.extends.clone());
    }
    None
}

/// Paramètres de `init` (déclaré ou hérité), ou constructeur généré d'un `struct`.
fn constructor_params(program: &Program, class: &str) -> Option<Vec<Param>> {
    find_member(program, class, |_, m| match m {
        ClassMember::Constructor { params, .. } => Some(params.clone()),
        _ => None,
    })
}

fn user_call(owner: String, params: &[Param], returns: Option<String>) -> CallInfo {
    let params = params.iter().map(|p| ParamInfo {
        name: p.name.clone(),
        label: param_label(p),
        variadic: p.is_variadic,
        optional: p.default_value.is_some(),
    }).collect();
    CallInfo { owner, params, returns }
}
