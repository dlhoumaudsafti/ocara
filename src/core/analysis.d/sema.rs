//! Analyse sémantique et conversion de ses erreurs et avertissements en
//! diagnostics (triés par position, contexte runtime résolu).

use std::path::{Path, PathBuf};

use crate::core::diagnostics::{Diagnostic, Severity};
use crate::core::runtime_expand::{get_stmt_end_line, get_stmt_start_line};
use crate::parsing::ast::Program;
use crate::parsing::token::Span;
use crate::sema::index::Reference;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

pub struct SemaOutput {
    pub diagnostics: Vec<Diagnostic>,
    pub rewrites: crate::sema::named_args::AstRewrites,
    pub references: Vec<Reference>,
}

pub fn run(program: &Program, symbols: &SymbolTable, input: &Path, index: bool) -> SemaOutput {
    let mut checker = TypeChecker::new(symbols);
    if index {
        checker.index = Some(Vec::new());
    }
    checker.check_program(program);

    let mut items: Vec<(Span, Severity, String)> = Vec::new();
    items.extend(checker.errors.iter().map(|e| (e.span().clone(), Severity::Error, e.message())));
    items.extend(checker.warnings.iter().map(|w| (w.span().clone(), Severity::Warning, w.message())));
    items.sort_by_key(|(span, _, _)| (span.line, span.col));

    let ranges = runtime_ranges(program, input);
    let diagnostics = items.into_iter().map(|(span, severity, message)| {
        let file = span.file.as_ref().map(PathBuf::from).unwrap_or_else(|| input.to_path_buf());
        let runtime_ctx = span.runtime_ctx.clone().or_else(|| {
            ranges.iter()
                .find(|(f, r, _)| same_file(f, &file) && r.contains(&span.line))
                .map(|(_, _, k)| k.to_string())
        });
        Diagnostic { file, line: span.line, col: span.col, message, severity, runtime_ctx }
    }).collect();

    SemaOutput {
        diagnostics,
        rewrites: std::mem::take(&mut checker.rewrites),
        references: checker.index.take().unwrap_or_default(),
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    a == b || a.canonicalize().ok() == b.canonicalize().ok()
}

/// Lignes de chaque bloc runtime, dans son fichier (le principal, ou le
/// fichier runtime importé dont il vient).
fn runtime_ranges<'a>(program: &'a Program, input: &Path) -> Vec<(PathBuf, std::ops::Range<usize>, &'a str)> {
    let mut out = Vec::new();
    for block in &program.runtime_blocks {
        // Un bloc fusionne les instructions de plusieurs fichiers : une plage
        // par suite d'instructions consécutives d'un même fichier.
        let mut groups: Vec<(PathBuf, usize, usize)> = Vec::new();
        for stmt in &block.statements {
            let file = crate::core::runtime_expand::get_stmt_span(stmt).file.as_ref().map(PathBuf::from).unwrap_or_else(|| input.to_path_buf());
            let (start, end) = (get_stmt_start_line(stmt), get_stmt_end_line(stmt));
            if start == 0 || end == 0 { continue; }
            match groups.last_mut() {
                Some((f, _, e)) if *f == file => *e = (*e).max(end),
                _ => groups.push((file, start, end)),
            }
        }
        out.extend(groups.into_iter().map(|(f, s, e)| (f, s..e + 1, block.kind.as_str())));
    }
    out
}
