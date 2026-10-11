//! Constantes de build (`System::OS`, `System::ARCH`, `System::BUILD`) qui
//! choisissent les variantes `when` au parsing, et journal de cette
//! sélection relu par l'analyse (voir core::analysis::variants et
//! docs/roadmap.d/reflexion-declarations-conditionnelles-when.md).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use crate::parsing::token::Span;

/// Valeurs possibles de chaque constante.
pub const DOMAINS: &[(&str, &[&str])] = &[
    ("OS", &["linux", "windows", "macos", "android"]),
    ("ARCH", &["x86_64", "aarch64", "arm"]),
    ("BUILD", &["debug", "release"]),
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BuildConfig {
    pub values: BTreeMap<String, String>,
    /// Faux : toutes les variantes sont gardées (index du serveur de langage).
    pub select: bool,
}

impl BuildConfig {
    /// Cible du build : triple `--target` (hôte sinon), mode `--release`.
    pub fn for_target(target: Option<&str>, release: bool) -> Self {
        let (os, arch) = match target {
            Some(t) => (target_os(t), t.split('-').next().unwrap_or("").to_string()),
            None => (std::env::consts::OS.to_string(), std::env::consts::ARCH.to_string()),
        };
        let build = if release { "release" } else { "debug" };
        let values = [("OS", os), ("ARCH", arch), ("BUILD", build.to_string())]
            .into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        Self { values, select: true }
    }

    pub fn keep_all() -> Self {
        Self { select: false, ..Self::for_target(None, false) }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// `System::OS = 'windows', System::BUILD = 'release'` (constantes `names`).
    pub fn describe(&self, names: &BTreeSet<String>) -> String {
        names.iter()
            .filter_map(|n| self.get(n).map(|v| format!("System::{} = '{}'", n, v)))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn target_os(triple: &str) -> String {
    let os = if triple.contains("android") {
        "android"
    } else if triple.contains("windows") {
        "windows"
    } else if triple.contains("apple") {
        "macos"
    } else {
        "linux"
    };
    os.to_string()
}

/// Variante `when` rencontrée : retenue ou non pour la configuration courante.
#[derive(Debug, Clone)]
pub struct VariantNote {
    pub file: String,
    /// Clause `when` (ou déclaration d'une variante sans clause).
    pub span: Span,
    pub end_line: usize,
    pub retained: bool,
}

#[derive(Debug, Default)]
pub struct SelectionLog {
    /// Constantes lues par les clauses, et valeurs comparées.
    pub used: BTreeMap<String, BTreeSet<String>>,
    pub variants: Vec<VariantNote>,
    /// Avertissements (fichier, position, message).
    pub warnings: Vec<(String, Span, String)>,
}

thread_local! {
    static CONFIG: RefCell<BuildConfig> = RefCell::new(BuildConfig::for_target(None, false));
    static FILE: RefCell<Option<String>> = const { RefCell::new(None) };
    static LOG: RefCell<SelectionLog> = RefCell::new(SelectionLog::default());
}

pub fn current() -> BuildConfig {
    CONFIG.with(|c| c.borrow().clone())
}

pub fn set_current(config: BuildConfig) {
    CONFIG.with(|c| *c.borrow_mut() = config);
}

/// Exécute `f` avec la configuration `config`, puis rétablit la précédente.
pub fn with_config<T>(config: BuildConfig, f: impl FnOnce() -> T) -> T {
    let previous = CONFIG.with(|c| std::mem::replace(&mut *c.borrow_mut(), config));
    let out = f();
    set_current(previous);
    out
}

/// Fichier en cours de parsing, pour situer le journal ; un parsing hors
/// de ce cadre (fichier inconnu) n'est pas journalisé.
pub fn parsing_file<T>(file: &str, f: impl FnOnce() -> T) -> T {
    let previous = FILE.with(|c| c.borrow_mut().replace(file.to_string()));
    let out = f();
    FILE.with(|c| *c.borrow_mut() = previous);
    out
}

pub fn take_log() -> SelectionLog {
    LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub(crate) fn note_used(name: &str, value: Option<&str>) {
    LOG.with(|l| {
        let mut log = l.borrow_mut();
        let values = log.used.entry(name.to_string()).or_default();
        if let Some(v) = value {
            values.insert(v.to_string());
        }
    });
}

pub(crate) fn note_variant(span: &Span, end_line: usize, retained: bool) {
    let Some(file) = FILE.with(|c| c.borrow().clone()) else { return };
    LOG.with(|l| l.borrow_mut().variants.push(VariantNote { file, span: span.clone(), end_line: end_line.max(span.line), retained }));
}

pub(crate) fn note_warning(span: &Span, message: String) {
    let Some(file) = FILE.with(|c| c.borrow().clone()) else { return };
    LOG.with(|l| l.borrow_mut().warnings.push((file, span.clone(), message)));
}
