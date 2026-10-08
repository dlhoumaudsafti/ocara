//! Complétion : la sema analyse le texte patché (`callsite::patched_text`)
//! et rapporte, au marqueur, le type réel du receveur (`a.`, `A::`) ou les
//! noms visibles ; les membres viennent des déclarations et du catalogue
//! builtin.

use std::collections::HashSet;
use std::path::Path;

use lsp_types::{
    CompletionItem, CompletionItemKind, Documentation, InsertTextFormat, MarkupContent, MarkupKind, Position,
};

use super::builtin_docs::{self, BuiltinMethod};
use super::callsite::{call_site, patched_text};
use super::decls::{display_type, function_signature};
use super::signature::named_argument_items;
use super::workspace::Workspace;
use crate::parsing::ast::{ClassMember, Param, Program, Type};
use crate::sema::convert_sugar::convert_method_for;
use crate::sema::index::{Completion, Target, COMPLETION_MARKER};

/// Propriétés communes aux exceptions builtin (voir src/builtins/exception.rs).
const EXCEPTION_PROPERTIES: &[(&str, &str)] = &[("message", "string"), ("code", "int"), ("source", "string")];

/// Classes builtin dont les méthodes statiques prenant une instance en
/// premier paramètre s'appellent sur l'instance (`req.path()`), comme
/// `allows_instance_sugar` dans src/sema/typecheck.rs.
const INSTANCE_SUGAR_CLASSES: &[&str] = &["HTTPRequest", "HTTPResponse", "HTTPServerRequest", "HTTPServerSession"];

pub fn completion(ws: &Workspace, path: &Path, pos: &Position) -> Vec<CompletionItem> {
    let Some(text) = ws.text(path) else { return Vec::new() };
    let line: String = text.lines().nth(pos.line as usize).unwrap_or("").chars().take(pos.character as usize).collect();
    if is_after_use(&line) {
        return class_items(ws.analysis(path).and_then(|a| a.checked.as_ref()).map(|c| &c.program));
    }

    let patched = patched_text(&text, pos);
    let analysis = ws.analyze_text(path, patched.clone());
    let Some(checked) = analysis.checked.as_ref() else { return Vec::new() };
    let Some(marker) = checked.references.iter().find(|r| r.name == COMPLETION_MARKER) else { return Vec::new() };
    let Target::Completion(kind) = &marker.target else { return Vec::new() };
    match kind {
        Completion::Member => members(&checked.program, &marker.ty, false),
        Completion::Static => members(&checked.program, &marker.ty, true),
        Completion::Scope(locals) => {
            let mut items = match call_site(&text, pos) {
                Some(site) => named_argument_items(&analysis, &patched, &site),
                None => Vec::new(),
            };
            items.extend(scope_items(&checked.program, locals));
            items
        }
    }
}

fn is_after_use(line: &str) -> bool {
    let trimmed = line.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
    trimmed.ends_with("use ") && trimmed.trim_end().split_whitespace().last() == Some("use")
}

// ── Membres d'un type ───────────────────────────────────────────────────────

fn members(program: &Program, ty: &Type, want_static: bool) -> Vec<CompletionItem> {
    let class = match ty {
        Type::Named(name) | Type::Generic { name, .. } => name.clone(),
        Type::Union(variants) => match variants.iter().find(|v| !matches!(v, Type::Null)) {
            Some(inner) => return members(program, inner, want_static),
            None => return Vec::new(),
        },
        primitive if !want_static => return primitive_members(primitive),
        _ => return Vec::new(),
    };
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    let mut current = Some(class.clone());
    while let Some(owner) = current.take() {
        let (members, parent) = if let Some(c) = program.classes.iter().find(|c| c.name == owner) {
            (Some(&c.members), c.extends.clone())
        } else if let Some(g) = program.generics.iter().find(|g| g.name == owner) {
            (Some(&g.members), g.extends.clone())
        } else {
            (None, None)
        };
        for m in members.into_iter().flatten() {
            if let Some(item) = member_item(&owner, m, want_static) {
                if seen.insert(item.label.clone()) { items.push(item); }
            }
        }
        if members.is_none() {
            for item in builtin_members(&owner, want_static) {
                if seen.insert(item.label.clone()) { items.push(item); }
            }
        }
        current = parent;
    }
    if !want_static && class.ends_with("Exception") && program.classes.iter().all(|c| c.name != class) {
        items.extend(EXCEPTION_PROPERTIES.iter().map(|(name, ty)| item(name, CompletionItemKind::FIELD, format!("{}.{}: {}", class, name, ty))));
    }
    items
}

fn member_item(class: &str, m: &ClassMember, want_static: bool) -> Option<CompletionItem> {
    match m {
        ClassMember::Method { decl, is_static, .. } if *is_static == want_static => {
            let sep = if *is_static { "::" } else { "." };
            Some(call_item(&decl.name, CompletionItemKind::METHOD, function_signature(&format!("{}{}", class, sep), decl), &param_names(&decl.params), None))
        }
        ClassMember::Field { name, ty, .. } if !want_static => Some(item(name, CompletionItemKind::FIELD, format!("{}.{}: {}", class, name, display_type(ty)))),
        ClassMember::Const { name, ty, .. } if want_static => Some(item(name, CompletionItemKind::CONSTANT, format!("{}::{}: {}", class, name, display_type(ty)))),
        _ => None,
    }
}

fn builtin_members(class: &str, want_static: bool) -> Vec<CompletionItem> {
    let Some(info) = builtin_docs::class(class) else { return Vec::new() };
    let mut items: Vec<CompletionItem> = info.methods.iter()
        .filter(|m| m.is_static == want_static)
        .map(|m| builtin_item(class, m, 0))
        .collect();
    if want_static {
        items.extend(info.consts.iter().map(|(name, ty)| item(name, CompletionItemKind::CONSTANT, format!("{}::{}: {}", class, name, ty))));
    } else if INSTANCE_SUGAR_CLASSES.contains(&class) {
        items.extend(info.methods.iter()
            .filter(|m| m.is_static && m.params.first().is_some_and(|(_, t)| t == class))
            .map(|m| builtin_item(class, m, 1)));
    }
    items
}

/// `string`, `int`… : conversions `Convert` (`s.toInt()`) et méthodes
/// `String`/`Array`/`Map` appelées sur la valeur (`s.trim()`).
fn primitive_members(ty: &Type) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    for name in ["toInt", "toFloat", "toBool", "toStr", "toArray", "toMap"] {
        let Some(target) = convert_method_for(ty, name) else { continue };
        if let Some(m) = builtin_docs::method("Convert", target) {
            let mut it = builtin_item("Convert", m, 1);
            it.label = name.to_string();
            it.insert_text = Some(snippet(name, &m.params.iter().skip(1).map(|(n, _)| n.clone()).collect::<Vec<_>>()));
            it.detail = Some(format!("{} — Convert::{}", it.detail.unwrap_or_default(), target));
            items.push(it);
        }
    }
    let sugar = match ty {
        Type::String => "String",
        Type::Array(_) => "Array",
        Type::Map(_, _) => "Map",
        _ => return items,
    };
    if let Some(info) = builtin_docs::class(sugar) {
        items.extend(info.methods.iter().filter(|m| m.is_static).map(|m| builtin_item(sugar, m, 1)));
    }
    items
}

fn builtin_item(class: &str, m: &BuiltinMethod, skip: usize) -> CompletionItem {
    let params: Vec<String> = m.params.iter().skip(skip).map(|(n, _)| n.clone()).collect();
    let shown: Vec<String> = m.params.iter().skip(skip).map(|(n, t)| format!("{}:{}", n, t)).collect();
    let sep = if m.is_static && skip == 0 { "::" } else { "." };
    let detail = format!("{}{}{}({}): {}", class, sep, m.name, shown.join(", "), m.returns);
    call_item(&m.name, CompletionItemKind::METHOD, detail, &params, m.doc.clone())
}

// ── Noms visibles ───────────────────────────────────────────────────────────

fn scope_items(program: &Program, locals: &[(String, Type)]) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = locals.iter()
        .map(|(name, ty)| item(name, CompletionItemKind::VARIABLE, format!("{}: {}", name, display_type(ty))))
        .collect();
    items.extend(program.functions.iter().map(|f| {
        call_item(&f.name, CompletionItemKind::FUNCTION, function_signature("function ", f), &param_names(&f.params), None)
    }));
    items.extend(program.consts.iter().map(|c| item(&c.name, CompletionItemKind::CONSTANT, format!("const {}: {}", c.name, display_type(&c.ty)))));
    items.extend(program.classes.iter().map(|c| item(&c.name, CompletionItemKind::CLASS, format!("class {}", c.name))));
    items
}

/// `use ` : classes utilisateur et builtin instanciables.
fn class_items(program: Option<&Program>) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for c in program.into_iter().flat_map(|p| &p.classes) {
        if seen.insert(c.name.clone()) {
            let params = c.members.iter().find_map(|m| match m { ClassMember::Constructor { params, .. } => Some(param_names(params)), _ => None }).unwrap_or_default();
            items.push(call_item(&c.name, CompletionItemKind::CLASS, format!("class {}", c.name), &params, None));
        }
    }
    for name in crate::core::analysis::OCARA_BUILTINS {
        let instantiable = builtin_docs::class(name).is_some_and(|c| c.methods.iter().any(|m| !m.is_static));
        if instantiable && seen.insert(name.to_string()) {
            items.push(call_item(name, CompletionItemKind::CLASS, format!("ocara.{}", name), &[], None));
        }
    }
    items
}

// ── Construction des éléments ───────────────────────────────────────────────

fn param_names(params: &[Param]) -> Vec<String> {
    params.iter().map(|p| p.name.clone()).collect()
}

fn item(label: &str, kind: CompletionItemKind, detail: String) -> CompletionItem {
    CompletionItem { label: label.to_string(), kind: Some(kind), detail: Some(detail), ..Default::default() }
}

/// Appel avec un champ à remplir par paramètre, nommé comme lui.
fn call_item(name: &str, kind: CompletionItemKind, detail: String, params: &[String], doc: Option<String>) -> CompletionItem {
    let documentation = format!("```ocara\n{}\n```{}", detail, doc.map(|d| format!("\n\n{}", d)).unwrap_or_default());
    CompletionItem {
        label: name.to_string(),
        kind: Some(kind),
        detail: Some(detail),
        documentation: Some(Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown, value: documentation })),
        insert_text: Some(snippet(name, params)),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    }
}

fn snippet(name: &str, params: &[String]) -> String {
    let escape = |t: &str| t.replace('\\', "\\\\").replace('$', "\\$").replace('}', "\\}");
    let fields: Vec<String> = params.iter().enumerate().map(|(i, p)| format!("${{{}:{}}}", i + 1, escape(p))).collect();
    format!("{}({})", name, fields.join(", "))
}
