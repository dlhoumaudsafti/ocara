//! Réponses aux requêtes : diagnostics, survol, définition, symboles.

use std::path::{Path, PathBuf};

use lsp_types::{
    DiagnosticRelatedInformation, DiagnosticSeverity, DocumentSymbol, Hover, HoverContents, Location,
    MarkupContent, MarkupKind, Position, Range, SymbolKind, Url,
};

use super::builtin_docs;
use super::decls::{self, display_type};
use super::position::{contains, name_range, point};
use super::workspace::Workspace;
use crate::core::analysis::Analysis;
use crate::core::diagnostics::{Diagnostic, Severity};
use crate::parsing::ast::{ClassMember, Program};
use crate::parsing::token::Span;
use crate::sema::index::{Reference, Target};

pub fn to_url(path: &Path) -> Url {
    Url::from_file_path(path).unwrap_or_else(|_| Url::parse("file:///").unwrap())
}

pub fn span_path(span: &Span, entry: &Path) -> PathBuf {
    span.file.as_ref().map(|f| PathBuf::from(f).canonicalize().unwrap_or_else(|_| PathBuf::from(f))).unwrap_or_else(|| entry.to_path_buf())
}

// ── Diagnostics ─────────────────────────────────────────────────────────────

/// Diagnostics du document `path` : les siens, et ceux d'un fichier importé
/// rapportés en tête du document (le programme ne compile pas à cause d'eux).
pub fn diagnostics(ws: &Workspace, path: &Path, analysis: &Analysis) -> Vec<lsp_types::Diagnostic> {
    let text = ws.text(path).unwrap_or_default();
    analysis.diagnostics.iter().map(|d| {
        let own = d.file.canonicalize().unwrap_or_else(|_| d.file.clone()) == path;
        if own {
            let span = Span::new(d.line.max(1), d.col.max(1));
            let mut range = name_range(&text, &span, &word_at(&text, &span));
            if range.start == range.end {
                range.end.character += 1;
            }
            to_lsp(d, range, d.message.clone(), None)
        } else {
            let other = d.file.canonicalize().unwrap_or_else(|_| d.file.clone());
            let related = DiagnosticRelatedInformation {
                location: Location::new(to_url(&other), point(d.line.saturating_sub(1), d.col.saturating_sub(1))),
                message: d.message.clone(),
            };
            let message = format!("{}:{}:{}: {}", other.display(), d.line, d.col, d.message);
            to_lsp(d, Range::new(Position::new(0, 0), Position::new(0, 1)), message, Some(vec![related]))
        }
    }).collect()
}

fn to_lsp(d: &Diagnostic, range: Range, message: String, related: Option<Vec<DiagnosticRelatedInformation>>) -> lsp_types::Diagnostic {
    let message = match &d.runtime_ctx {
        Some(ctx) => format!("[{}] {}", ctx, message),
        None => message,
    };
    lsp_types::Diagnostic {
        range,
        severity: Some(match d.severity { Severity::Error => DiagnosticSeverity::ERROR, Severity::Warning => DiagnosticSeverity::WARNING }),
        source: Some("ocara".into()),
        message,
        related_information: related,
        ..Default::default()
    }
}

/// Identifiant qui commence à la position du span (pour surligner le nom).
fn word_at(text: &str, span: &Span) -> String {
    text.lines().nth(span.line - 1)
        .map(|l| l.chars().skip(span.col - 1).take_while(|c| c.is_alphanumeric() || *c == '_').collect())
        .unwrap_or_default()
}

// ── Référence sous le curseur ───────────────────────────────────────────────

pub fn reference_at(ws: &Workspace, analysis: &Analysis, path: &Path, pos: &Position) -> Option<(Reference, Range)> {
    let checked = analysis.checked.as_ref()?;
    let text = ws.text(path)?;
    let here = |span: &Span| span_path(span, path) == path;
    let direct = checked.references.iter()
        .filter(|r| here(&r.span))
        .map(|r| (r, name_range(&text, &r.span, &r.name)))
        .find(|(_, range)| contains(range, pos));
    // Curseur sur la déclaration d'une variable : connue par ses utilisations.
    let declared = || checked.references.iter()
        .filter_map(|r| match &r.target { Target::Local { decl } if here(decl) => Some((r, name_range(&text, decl, &r.name))), _ => None })
        .find(|(_, range)| contains(range, pos));
    if let Some((r, range)) = direct.or_else(declared) {
        return Some((r.clone(), range));
    }
    type_reference_at(&parse_document(ws, path)?, &text, pos)
}

/// Nom de type écrit dans le document (`var x:Dog`, `extends Animal`…) :
/// référence à la classe qu'il désigne.
fn type_reference_at(own: &Program, text: &str, pos: &Position) -> Option<(Reference, Range)> {
    own.type_refs.iter()
        .map(|(name, span)| (name, span, name_range(text, span, name)))
        .find(|(_, _, range)| contains(range, pos))
        .map(|(name, span, range)| (Reference {
            span: span.clone(),
            name: name.clone(),
            target: Target::Class(name.clone()),
            ty: crate::parsing::ast::Type::Named(name.clone()),
        }, range))
}

// ── Survol ──────────────────────────────────────────────────────────────────

pub fn hover(ws: &Workspace, path: &Path, pos: &Position) -> Option<Hover> {
    let analysis = ws.analysis(path)?;
    let Some((reference, range)) = reference_at(ws, analysis, path, pos) else {
        return keyword_hover(ws, path, pos);
    };
    let program = &analysis.checked.as_ref()?.program;
    let markdown = hover_markdown(ws, program, &reference, path)?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: markdown }),
        range: Some(range),
    })
}

/// Mot-clé sous le curseur, hors chaîne et commentaire.
fn keyword_hover(ws: &Workspace, path: &Path, pos: &Position) -> Option<Hover> {
    let text = ws.text(path)?;
    let line: Vec<char> = text.lines().nth(pos.line as usize)?.chars().collect();
    let at = (pos.character as usize).min(line.len());
    let is_word = |c: &char| c.is_alphanumeric() || *c == '_';
    let start = (0..at).rev().take_while(|&i| is_word(&line[i])).last().unwrap_or(at);
    let end = (at..line.len()).find(|&i| !is_word(&line[i])).unwrap_or(line.len());
    if start == end || !super::callsite::is_code_at(&text, pos) {
        return None;
    }
    let word: String = line[start..end].iter().collect();
    let before: String = line[..start].iter().collect();
    let after: String = line[end..].iter().collect();
    let markdown = super::keywords::doc(&word, &before, &after)?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: markdown }),
        range: Some(Range::new(Position::new(pos.line, start as u32), Position::new(pos.line, end as u32))),
    })
}

fn hover_markdown(ws: &Workspace, program: &Program, r: &Reference, entry: &Path) -> Option<String> {
    if let Target::Local { .. } = r.target {
        return Some(code(&format!("{}:{}", r.name, display_type(&r.ty))));
    }
    if let Some(decl) = decls::find(program, &r.target) {
        let mut parts = vec![code(&decl.signature)];
        if let Some(comment) = leading_comment(ws, &span_path(&decl.span, entry), decl.span.line) {
            parts.push(comment);
        }
        return Some(parts.join("\n\n"));
    }
    match &r.target {
        Target::Method { class, name } => builtin_method(program, class, name),
        Target::Class(name) if builtin_docs::is_builtin_class(name) => Some(format!(
            "{}\n\nClasse builtin.\n\n{}", code(&format!("ocara.{}", name)), builtin_docs::doc_link(&format!("{}.md", name), None))),
        Target::ClassConst { class, name } => Some(code(&format!("{}::{}:{}", class, name, display_type(&r.ty)))),
        Target::Field { class, name } => Some(code(&format!("{}.{}:{}", class, name, display_type(&r.ty)))),
        _ => None,
    }
}

/// Méthode builtin, éventuellement héritée par une classe utilisateur.
fn builtin_method(program: &Program, class: &str, name: &str) -> Option<String> {
    let mut current = Some(class.to_string());
    while let Some(owner) = current.take() {
        if let Some(m) = builtin_docs::method(&owner, name) {
            let mut parts = vec![code(&m.signature(&owner))];
            parts.extend(m.doc.clone());
            if owner != class {
                parts.push(format!("_Hérité de `{}` par `{}`._", owner, class));
            }
            parts.push(builtin_docs::doc_link(&m.doc_file, m.doc_heading.as_deref()));
            return Some(parts.join("\n\n"));
        }
        current = program.classes.iter().find(|c| c.name == owner).and_then(|c| c.extends.clone());
    }
    None
}

/// Commentaires `//` contigus juste au-dessus de la ligne `line`.
fn leading_comment(ws: &Workspace, file: &Path, line: usize) -> Option<String> {
    let text = ws.text(file)?;
    let lines: Vec<&str> = text.lines().take(line.saturating_sub(1)).collect();
    let comment: Vec<&str> = lines.iter().rev()
        .map(|l| l.trim())
        .take_while(|l| l.starts_with("//"))
        .map(|l| l.trim_start_matches('/').trim())
        .collect();
    (!comment.is_empty()).then(|| comment.into_iter().rev().collect::<Vec<_>>().join("\n"))
}

fn code(s: &str) -> String {
    format!("```ocara\n{}\n```", s)
}

// ── Définition ──────────────────────────────────────────────────────────────

pub fn definition(ws: &Workspace, path: &Path, pos: &Position) -> Option<Location> {
    let analysis = ws.analysis(path)?;
    if let Some(location) = declaration_line_target(ws, analysis, path, pos) {
        return Some(location);
    }
    let (reference, _) = reference_at(ws, analysis, path, pos)?;
    let program = &analysis.checked.as_ref()?.program;
    let (span, name) = match &reference.target {
        Target::Local { decl } => (decl.clone(), reference.name.clone()),
        target => {
            let decl = decls::find(program, target)?;
            (decl.span, decl.name)
        }
    };
    let file = span_path(&span, path);
    let text = ws.text(&file)?;
    Some(Location::new(to_url(&file), name_range(&text, &span, &name)))
}

/// Lignes `import`, `runtime` et `wiring` du document : le fichier ou la
/// classe qu'elles désignent.
fn declaration_line_target(ws: &Workspace, analysis: &Analysis, path: &Path, pos: &Position) -> Option<Location> {
    let own = parse_document(ws, path)?;
    let on_line = |span: &Span| span.line == pos.line as usize + 1;
    let root = ws.project_root(path);
    let merged = analysis.checked.as_ref().map(|c| &c.program);

    if let Some(imp) = own.imports.iter().find(|i| on_line(&i.span)) {
        if imp.path.first().is_some_and(|p| p == "ocara") {
            return None;
        }
        let (relative, symbol) = match &imp.file_path {
            Some(file) => (file.clone(), imp.path.first().cloned().unwrap_or_default()),
            None => (imp.path.join("/"), imp.path.last().cloned().unwrap_or_default()),
        };
        let dir = path.parent().unwrap_or(&root);
        let parent = if imp.file_path.is_some() { dir } else { root.as_path() };
        let file = crate::core::analysis::resolve_import_path(&relative, parent, &own.namespace, &root);
        let file = file.canonicalize().unwrap_or(file);
        let in_file = merged.and_then(|p| decls::find(p, &Target::Class(symbol.clone()))
            .or_else(|| decls::find(p, &Target::Function(symbol.clone()))))
            .filter(|d| span_path(&d.span, path) == file);
        return Some(match in_file {
            Some(d) => location_of(ws, &file, &d.span, &d.name)?,
            None => Location::new(to_url(&file), point(0, 0)),
        });
    }
    if let Some(rt) = own.runtime_imports.iter().find(|r| on_line(&r.span)) {
        let file = crate::core::runtime_expand::resolve_runtime_file(&root, &rt.path)?;
        return Some(Location::new(to_url(&file.canonicalize().unwrap_or(file)), point(0, 0)));
    }
    let wiring = own.interfaces.iter().flat_map(|i| &i.wirings).find(|w| on_line(&w.span))?;
    let d = decls::find(merged?, &Target::Class(wiring.simple_name().to_string()))?;
    location_of(ws, &span_path(&d.span, path), &d.span, &d.name)
}

fn location_of(ws: &Workspace, file: &Path, span: &Span, name: &str) -> Option<Location> {
    let text = ws.text(file)?;
    Some(Location::new(to_url(file), name_range(&text, span, name)))
}

pub fn parse_document(ws: &Workspace, path: &Path) -> Option<Program> {
    let text = ws.text(path)?;
    let tokens = crate::parsing::lexer::Lexer::new(&text).tokenize().ok()?;
    crate::parsing::parser::Parser::new(tokens).parse_program().ok()
}

// ── Symboles du document ────────────────────────────────────────────────────

pub fn document_symbols(ws: &Workspace, path: &Path) -> Vec<DocumentSymbol> {
    let Some(text) = ws.text(path) else { return Vec::new() };
    let Some(program) = parse_document(ws, path) else { return Vec::new() };
    let sym = |name: &str, kind: SymbolKind, span: &Span, detail: Option<String>, children: Vec<DocumentSymbol>| {
        let range = name_range(&text, span, name);
        #[allow(deprecated)]
        DocumentSymbol {
            name: name.to_string(), detail, kind, tags: None, deprecated: None,
            range, selection_range: range,
            children: (!children.is_empty()).then_some(children),
        }
    };
    let members = |members: &[ClassMember]| -> Vec<DocumentSymbol> {
        members.iter().map(|m| match m {
            ClassMember::Field { name, ty, span, .. } => sym(name, SymbolKind::FIELD, span, Some(display_type(ty)), vec![]),
            ClassMember::Const { name, ty, span, .. } => sym(name, SymbolKind::CONSTANT, span, Some(display_type(ty)), vec![]),
            ClassMember::Method { decl, .. } => sym(&decl.name, SymbolKind::METHOD, &decl.span, None, vec![]),
            ClassMember::Constructor { span, .. } => sym("init", SymbolKind::CONSTRUCTOR, span, None, vec![]),
        }).collect()
    };
    let mut out = Vec::new();
    for c in &program.consts { out.push(sym(&c.name, SymbolKind::CONSTANT, &c.span, Some(display_type(&c.ty)), vec![])); }
    for e in &program.enums { out.push(sym(&e.name, SymbolKind::ENUM, &e.span, None, vec![])); }
    for i in &program.interfaces { out.push(sym(&i.name, SymbolKind::INTERFACE, &i.span, None, vec![])); }
    for m in &program.modules { out.push(sym(&m.name, SymbolKind::MODULE, &m.span, None, members(&m.members))); }
    for c in &program.classes {
        let kind = if c.is_struct { SymbolKind::STRUCT } else { SymbolKind::CLASS };
        out.push(sym(&c.name, kind, &c.span, None, members(&c.members)));
    }
    for g in &program.generics { out.push(sym(&g.name, SymbolKind::CLASS, &g.span, None, members(&g.members))); }
    for f in &program.functions { out.push(sym(&f.name, SymbolKind::FUNCTION, &f.span, None, vec![])); }
    out.sort_by_key(|s| s.range.start);
    out
}
