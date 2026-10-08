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

    let ranges = runtime_ranges(program);
    let diagnostics = items.into_iter().map(|(span, severity, message)| {
        let file = span.file.as_ref().map(PathBuf::from).unwrap_or_else(|| input.to_path_buf());
        let runtime_ctx = span.runtime_ctx.clone().or_else(|| {
            // Plages du fichier PRINCIPAL : jamais appliquées à un fichier importé.
            in_file(&file, input).then(|| ranges.iter().find(|(r, _)| r.contains(&span.line)).map(|(_, k)| k.to_string())).flatten()
        });
        Diagnostic { file, line: span.line, col: span.col, message, severity, runtime_ctx }
    }).collect();

    SemaOutput {
        diagnostics,
        rewrites: std::mem::take(&mut checker.rewrites),
        references: checker.index.take().unwrap_or_default(),
    }
}

fn in_file(file: &Path, input: &Path) -> bool {
    file == input || file.canonicalize().ok() == input.canonicalize().ok()
}

/// Lignes de chaque bloc runtime du fichier principal.
fn runtime_ranges(program: &Program) -> Vec<(std::ops::Range<usize>, &str)> {
    program.runtime_blocks.iter().filter_map(|block| {
        let (first, last) = (block.statements.first()?, block.statements.last()?);
        let (start, end) = (get_stmt_start_line(first), get_stmt_end_line(last));
        (start > 0 && end > 0).then(|| (start..end + 1, block.kind.as_str()))
    }).collect()
}
