//! Diagnostic produit par l'analyse (`core::analysis`) : affiché par le CLI
//! au format GCC (`parsing::diagnostic`), publié tel quel par le serveur de
//! langage (`lsp`).

use std::path::{Path, PathBuf};

use crate::parsing::diagnostic;
use crate::parsing::token::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub file: PathBuf,
    pub line: usize,
    pub col: usize,
    pub message: String,
    pub severity: Severity,
    /// Bloc runtime (`init`, `main`…) où se situe le diagnostic.
    pub runtime_ctx: Option<String>,
}

impl Diagnostic {
    pub fn error(file: &Path, line: usize, col: usize, message: impl Into<String>) -> Self {
        Self { file: file.to_path_buf(), line, col, message: message.into(), severity: Severity::Error, runtime_ctx: None }
    }

    /// Erreur située par un span : son fichier s'il est connu (déclaration
    /// d'un fichier importé), sinon `default_file`.
    pub fn at(default_file: &Path, span: &Span, message: impl Into<String>) -> Self {
        Self::error(&span_file(default_file, span), span.line, span.col, message)
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    pub fn print(&self) {
        let ctx = self.runtime_ctx.as_deref();
        match self.severity {
            Severity::Error => diagnostic::print_error_ctx(&self.file, self.line, self.col, &self.message, ctx),
            Severity::Warning => diagnostic::print_warn_ctx(&self.file, self.line, self.col, &self.message, ctx),
        }
    }
}

pub fn span_file(default_file: &Path, span: &Span) -> PathBuf {
    span.file.as_ref().map(PathBuf::from).unwrap_or_else(|| default_file.to_path_buf())
}
