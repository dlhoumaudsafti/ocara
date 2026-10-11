//! Imports manquants (`textDocument/codeAction`) : un nom de type utilisé
//! sans être déclaré ni importé (`HTTPServer::…`, `use Foo()`, `x:Foo`)
//! reçoit son import — `import ocara.X` pour un builtin, sinon le fichier du
//! projet qui le déclare, cherché dans le dossier du script, puis ses
//! sous-dossiers, puis chaque dossier parent (et ses sous-dossiers) jusqu'à
//! la racine du projet. Correctif rapide par nom, et action
//! `source.addMissingImports` pour tout le document (appliquée à
//! l'enregistrement par l'extension).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use lsp_types::{CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, Position, Range, TextEdit, WorkspaceEdit};

use super::features::to_url;
use super::project;
use super::workspace::Workspace;
use crate::core::analysis::{resolve_import_path, OCARA_BUILTINS};
use crate::parsing::ast::Program;
use crate::parsing::lexer::Lexer;
use crate::parsing::token::{TemplatePart, Token, TokenKind};

pub const SOURCE_KIND: &str = "source.addMissingImports";

/// Candidats proposés en correctif rapide pour un même nom.
const MAX_CANDIDATES: usize = 3;

pub fn code_actions(ws: &Workspace, path: &Path, params: &CodeActionParams) -> Vec<CodeActionOrCommand> {
    let wants = |kind: &CodeActionKind| params.context.only.as_ref()
        .is_none_or(|only| only.iter().any(|k| kind.as_str().starts_with(k.as_str())));
    let (quick, source) = (wants(&CodeActionKind::QUICKFIX), wants(&CodeActionKind::new(SOURCE_KIND)));
    if !quick && !source {
        return Vec::new();
    }
    let Some(text) = ws.text(path) else { return Vec::new() };
    let Some(program) = project::parse(&text) else { return Vec::new() };
    let mut search = Search::new(ws, path, &program);
    let missing = missing_names(&text, &program, &mut search);

    let mut actions = Vec::new();
    let lines = params.range.start.line as usize + 1..=params.range.end.line as usize + 1;
    let mut all = Vec::new();
    for (name, used_on) in &missing {
        let candidates = search.import_lines(name);
        let Some(best) = candidates.first() else { continue };
        all.push(best.clone());
        if quick && used_on.iter().any(|l| lines.contains(l)) {
            for line in &candidates {
                actions.push(action(format!("Ajouter « {} »", line), CodeActionKind::QUICKFIX, path, &text, &program, std::slice::from_ref(line), candidates.len() == 1));
            }
        }
    }
    if source && !all.is_empty() {
        actions.push(action("Ajouter les imports manquants".into(), CodeActionKind::new(SOURCE_KIND), path, &text, &program, &all, false));
    }
    actions
}

fn action(title: String, kind: CodeActionKind, path: &Path, text: &str, program: &Program, imports: &[String], preferred: bool) -> CodeActionOrCommand {
    let edit = insertion(text, program, imports);
    CodeActionOrCommand::CodeAction(CodeAction {
        title,
        kind: Some(kind),
        edit: Some(WorkspaceEdit { changes: Some(HashMap::from([(to_url(path), vec![edit])])), ..Default::default() }),
        is_preferred: Some(preferred),
        ..Default::default()
    })
}

/// Lignes `import` ajoutées après le dernier import, sinon après la ligne
/// `namespace`, sinon en tête du fichier.
fn insertion(text: &str, program: &Program, imports: &[String]) -> TextEdit {
    let block: String = imports.iter().map(|i| format!("{}\n", i)).collect();
    if let Some(last) = program.imports.iter().map(|i| i.span.line).max() {
        return TextEdit::new(point(last), block);
    }
    let namespace_line = text.lines().position(|l| l.trim_start().starts_with("namespace "));
    match namespace_line {
        Some(i) => TextEdit::new(point(i + 1), format!("\n{}", block)),
        None => TextEdit::new(point(0), format!("{}\n", block)),
    }
}

fn point(line: usize) -> Range {
    Range::new(Position::new(line as u32, 0), Position::new(line as u32, 0))
}

/// Noms de type (initiale majuscule) utilisés sans déclaration ni import,
/// avec les lignes où ils apparaissent.
fn missing_names(text: &str, program: &Program, search: &mut Search) -> Vec<(String, Vec<usize>)> {
    let Ok(tokens) = Lexer::new(text).tokenize() else { return Vec::new() };
    let known = search.known_names(program);
    let mut out: Vec<(String, Vec<usize>)> = Vec::new();
    scan(&tokens, 0, &known, &mut out);
    out
}

/// Noms de `tokens` (lignes décalées de `line_offset`), expressions des
/// gabarits backtick comprises.
fn scan(tokens: &[Token], line_offset: usize, known: &HashSet<String>, out: &mut Vec<(String, Vec<usize>)>) {
    let mut skipped_line = 0;
    for (i, tok) in tokens.iter().enumerate() {
        let line = tok.span.line + line_offset;
        if let TokenKind::LitTemplate(parts) = &tok.kind {
            for part in parts {
                if let TemplatePart::ExprSrc(src, span) = part {
                    if let Ok(inner) = Lexer::new(src).tokenize() {
                        scan(&inner, span.line - 1, known, out);
                    }
                }
            }
            continue;
        }
        let first_on_line = i == 0 || tokens[i - 1].span.line != line;
        if first_on_line && matches!(tok.kind, TokenKind::Import | TokenKind::Namespace | TokenKind::Runtime) {
            skipped_line = line;
        }
        let TokenKind::Ident(name) = &tok.kind else { continue };
        let after_access = i > 0 && matches!(tokens[i - 1].kind, TokenKind::Dot | TokenKind::ColonColon);
        if line == skipped_line || after_access || !name.starts_with(|c: char| c.is_ascii_uppercase()) || known.contains(name) {
            continue;
        }
        match out.iter_mut().find(|(n, _)| n == name) {
            Some((_, lines)) => lines.push(line),
            None => out.push((name.clone(), vec![line])),
        }
    }
}

/// Recherche des déclarations dans les fichiers du projet, du plus proche
/// au plus lointain ; fichiers lus une seule fois par requête.
struct Search<'a> {
    ws: &'a Workspace,
    path: PathBuf,
    root: PathBuf,
    namespace: Option<String>,
    order: Option<Vec<PathBuf>>,
    declared: HashMap<PathBuf, Vec<String>>,
}

impl<'a> Search<'a> {
    fn new(ws: &'a Workspace, path: &Path, program: &Program) -> Self {
        Self { ws, path: path.to_path_buf(), root: ws.project_root(path), namespace: program.namespace.clone(), order: None, declared: HashMap::new() }
    }

    /// Noms déjà visibles : déclarés ici, importés, paramètres de type.
    fn known_names(&mut self, program: &Program) -> HashSet<String> {
        let mut known: HashSet<String> = types_of(program).into_iter().collect();
        known.extend(program.generics.iter().flat_map(|g| g.type_params.iter().map(|p| p.name.clone())));
        for imp in &program.imports {
            let last = imp.path.last().cloned().unwrap_or_default();
            if let Some(alias) = &imp.alias {
                known.insert(alias.clone());
            }
            match (imp.path.first().map(String::as_str), last.as_str(), &imp.file_path) {
                (Some("ocara"), "*", _) => known.extend(OCARA_BUILTINS.iter().map(|s| s.to_string())),
                (_, "*", Some(file)) => {
                    let dir = self.path.parent().unwrap_or(&self.root).to_path_buf();
                    let target = resolve_import_path(file, &dir, &self.namespace, &self.root);
                    known.extend(self.declared_in(&target));
                }
                _ => { known.insert(last); }
            }
        }
        known
    }

    /// Lignes `import` possibles pour `name`, la plus proche d'abord.
    fn import_lines(&mut self, name: &str) -> Vec<String> {
        if OCARA_BUILTINS.contains(&name) {
            return vec![format!("import ocara.{}", name)];
        }
        let order = self.order.get_or_insert_with(|| search_order(&self.path, &self.root)).clone();
        let mut out = Vec::new();
        for file in order {
            if self.declared_in(&file).iter().any(|n| n == name) {
                out.push(self.import_line(name, &file));
                if out.len() == MAX_CANDIDATES {
                    break;
                }
            }
        }
        out
    }

    fn declared_in(&mut self, file: &Path) -> Vec<String> {
        let key = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
        if let Some(names) = self.declared.get(&key) {
            return names.clone();
        }
        let names = self.ws.text(&key).and_then(|t| project::parse(&t)).map(|p| types_of(&p)).unwrap_or_default();
        self.declared.insert(key, names.clone());
        names
    }

    /// `import a.b.Nom` si le fichier `a/b/Nom.oc` (depuis la racine) le
    /// déclare et que cette forme y mène, sinon `import Nom from "./…"`.
    fn import_line(&self, name: &str, file: &Path) -> String {
        let dir = self.path.parent().unwrap_or(&self.root).to_path_buf();
        if let Ok(rel) = file.strip_prefix(&self.root) {
            let rel = rel.with_extension("");
            let qualified = rel.to_string_lossy().replace('\\', "/");
            let plain = qualified.split('/').all(|seg| !seg.is_empty() && seg.chars().all(|c| c.is_alphanumeric() || c == '_'));
            let resolved = resolve_import_path(&qualified, &dir, &self.namespace, &self.root);
            if plain && rel.file_name().is_some_and(|f| f == name) && same_file(&resolved, file) {
                return format!("import {}", qualified.replace('/', "."));
            }
        }
        format!("import {} from \"{}\"", name, relative(&dir, &file.with_extension("")))
    }
}

fn types_of(program: &Program) -> Vec<String> {
    program.classes.iter().map(|c| c.name.clone())
        .chain(program.generics.iter().map(|g| g.name.clone()))
        .chain(program.interfaces.iter().map(|i| i.name.clone()))
        .chain(program.modules.iter().map(|m| m.name.clone()))
        .chain(program.enums.iter().map(|e| e.name.clone()))
        .collect()
}

fn same_file(a: &Path, b: &Path) -> bool {
    a.canonicalize().ok().is_some_and(|a| b.canonicalize().ok().is_some_and(|b| a == b))
}

/// Fichiers `.oc` du projet dans l'ordre de recherche : dossier du script,
/// ses sous-dossiers, puis chaque parent et ses autres sous-dossiers,
/// jusqu'à la racine ; à niveau égal, le moins profond d'abord.
fn search_order(path: &Path, root: &Path) -> Vec<PathBuf> {
    let me = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let dir = me.parent().unwrap_or(root).to_path_buf();
    let mut out: Vec<PathBuf> = Vec::new();
    let push_sorted = |mut files: Vec<PathBuf>, out: &mut Vec<PathBuf>| {
        files.retain(|f| *f != me && !out.contains(f));
        files.sort_by_key(|f| (f.components().count(), f.clone()));
        out.extend(files);
    };
    let mut here = Vec::new();
    project::collect_sources(&dir, &mut here);
    let (direct, nested): (Vec<_>, Vec<_>) = here.into_iter().partition(|f| f.parent() == Some(dir.as_path()));
    push_sorted(direct, &mut out);
    push_sorted(nested, &mut out);
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    for ancestor in dir.ancestors().skip(1) {
        if !ancestor.starts_with(&root) {
            break;
        }
        let mut files = Vec::new();
        project::collect_sources(ancestor, &mut files);
        push_sorted(files, &mut out);
    }
    out
}

/// Chemin de `target` depuis `dir`, toujours préfixé `./` ou `../`.
fn relative(dir: &Path, target: &Path) -> String {
    let dir: Vec<_> = dir.components().collect();
    let target_parts: Vec<_> = target.components().collect();
    let common = dir.iter().zip(&target_parts).take_while(|(a, b)| a == b).count();
    let ups = dir.len() - common;
    let rest: Vec<String> = target_parts[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    let prefix = if ups == 0 { "./".to_string() } else { "../".repeat(ups) };
    format!("{}{}", prefix, rest.join("/"))
}

#[cfg(test)]
mod tests {
    use super::relative;
    use std::path::Path;

    #[test]
    fn relative_paths_are_prefixed() {
        assert_eq!(relative(Path::new("/p/a/b"), Path::new("/p/a/b/c/X")), "./c/X");
        assert_eq!(relative(Path::new("/p/a/b"), Path::new("/p/a/Y")), "../Y");
        assert_eq!(relative(Path::new("/p/a/b"), Path::new("/p/z/W")), "../../z/W");
    }
}
