//! Analyse d'un programme, du fichier d'entrée à la sema : lecture, parsing,
//! imports, vérifications de structure, désucrages, analyse sémantique.
//! Partagée par le CLI (`main.rs`) et le serveur de langage (`lsp`) : aucune
//! étape ne quitte le processus, chaque erreur devient un `Diagnostic`.

mod checks;
mod front;
pub mod homonyms;
mod imports;
mod sema;
mod variants;

pub use imports::{resolve_import_path, OCARA_BUILTINS};

use std::path::Path;

use crate::core::alias_resolve::{compute_aliases, resolve_aliases};
use crate::core::diagnostics::Diagnostic;
use crate::parsing::ast::Program;
use crate::sema::index::Reference;
use crate::sema::symbols::SymbolTable;

pub struct AnalyzeOptions<'a> {
    pub input: &'a Path,
    /// Racine de résolution des imports (`--src`) ; dossier du fichier sinon.
    pub src_dir: Option<&'a Path>,
    /// Affiche les tokens et l'AST (`--dump`).
    pub dump: bool,
    /// Collecte l'index des références (serveur de langage).
    pub index: bool,
    /// Reprise sur erreur de syntaxe : analyse du programme partiel, seules
    /// les erreurs de syntaxe sont rapportées (serveur de langage).
    pub tolerant: bool,
}

/// Programme fusionné et vérifié par la sema (même avec des erreurs).
pub struct Checked {
    pub program: Program,
    /// Noms internes des types homonymes (nom écrit → nom interne, par fichier).
    pub homonyms: homonyms::Homonyms,
    pub references: Vec<Reference>,
}

pub struct Analysis {
    pub diagnostics: Vec<Diagnostic>,
    /// `None` si une étape avant la sema a échoué.
    pub checked: Option<Checked>,
}

impl Analysis {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(Diagnostic::is_error)
    }

    fn fatal(diagnostic: Diagnostic) -> Self {
        Self { diagnostics: vec![diagnostic], checked: None }
    }
}

/// Analyse pour la configuration de build courante (`parsing::build`) ;
/// les variantes `when` des autres configurations sont aussi vérifiées.
pub fn analyze(opts: &AnalyzeOptions) -> Analysis {
    variants::analyze_all(|| analyze_once(opts))
}

fn analyze_once(opts: &AnalyzeOptions) -> Analysis {
    let mut analysis = match analyze_program(opts) {
        Ok(analysis) => analysis,
        Err(diagnostic) => Analysis::fatal(diagnostic),
    };
    // Messages : noms internes des types homonymes réduits au nom écrit.
    for d in &mut analysis.diagnostics {
        d.message = homonyms::display(&d.message);
    }
    analysis
}

fn analyze_program(opts: &AnalyzeOptions) -> Result<Analysis, Diagnostic> {
    let input = opts.input;
    let (mut program, mut syntax) = front::parse_entry(input, opts.dump, opts.tolerant)?;
    let source_dir = opts.src_dir
        .unwrap_or_else(|| input.parent().unwrap_or_else(|| Path::new(".")));

    // Interfaces et leurs `wiring` atteignables, connues avant toute
    // résolution d'alias (voir core::interface_wiring).
    let all_interfaces = crate::core::interface_wiring::collect_all_interfaces(&program, source_dir);
    let aliases = compute_aliases(&program.imports, &all_interfaces, input)?;
    resolve_aliases(&mut program, &aliases);
    // Types homonymes de fichiers différents : un nom interne chacun.
    let homonyms = homonyms::Homonyms::scan(&program, input, source_dir);
    homonyms.apply(&mut program, input);

    imports::check_imports(&program, source_dir, input)?;
    syntax.extend(imports::load_imports(&mut program, source_dir, input, &all_interfaces, &homonyms, opts.tolerant)?);

    crate::core::interface_wiring::resolve_bare_interface_names(&mut program, &all_interfaces);
    crate::core::property_init::complete_implicit_inits(&mut program);
    crate::core::structs::expand_structs(&mut program)
        .map_err(|(span, msg)| Diagnostic::at(input, &span, msg))?;

    let symbols = build_symbols(&program);
    checks::check_extends(&program, &symbols, input)?;
    checks::check_implements(&program, &symbols, input)?;
    checks::check_wirings(&program, &symbols, input)?;

    crate::core::runtime_expand::expand_runtime_imports(&mut program, source_dir, input)?;
    // Chaque position connaît son fichier (déclarations importées et
    // instructions des fichiers runtime l'ont déjà).
    crate::core::runtime_expand::update_program_spans_with_file(&mut program, &input.to_string_lossy());
    // Le gabarit est lu à la compilation et réécrit en littéral avant la
    // sema, qui voit les mêmes expressions qu'un template backtick.
    crate::core::render_file::desugar_render_file(&mut program)
        .map_err(|(span, msg)| Diagnostic::at(input, &span, msg))?;

    let out = sema::run(&program, &symbols, input, opts.index);
    // Sur un programme amputé par des erreurs de syntaxe, les erreurs de la
    // sema seraient du bruit : seules les premières sont rapportées.
    let mut diagnostics = if syntax.is_empty() { out.diagnostics } else { syntax };
    if !diagnostics.iter().any(Diagnostic::is_error) {
        if let Err((span, msg)) = crate::core::named_args::rewrite_program(&mut program, &out.rewrites) {
            diagnostics.push(Diagnostic::at(input, &span, msg));
        }
    }
    Ok(Analysis {
        diagnostics,
        checked: Some(Checked { program, homonyms, references: out.references }),
    })
}

fn build_symbols(program: &Program) -> SymbolTable {
    let mut symbols = SymbolTable::new();
    for decl in &program.imports    { symbols.register_import(decl); }
    for decl in &program.consts     { symbols.register_const(decl); }
    for decl in &program.interfaces { symbols.register_interface(decl); }
    for decl in &program.modules    { symbols.register_module(decl); }
    for decl in &program.enums      { symbols.register_enum(decl); }
    for decl in &program.classes    { symbols.register_class(decl); }
    for decl in &program.generics   { symbols.register_generic(decl); }
    for decl in &program.functions  { symbols.register_function(decl); }
    symbols
}
