//! Variantes `when` : le programme est analysé pour la configuration de
//! build courante, puis une fois par autre combinaison des constantes lues
//! par les clauses (option A) — chaque variante est vérifiée par la sema.
//! Les diagnostics d'une autre combinaison portent sa description ; une
//! variante retenue par aucune combinaison est signalée.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;

use crate::core::diagnostics::{Diagnostic, Severity};
use crate::parsing::build::{self, BuildConfig, SelectionLog, DOMAINS};

use super::Analysis;

/// Lignes d'une variante écartée (clause comprise), 1-based.
#[derive(Debug, Clone)]
pub struct InactiveRange {
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
}

/// Au-delà, les combinaisons suivantes ne sont pas analysées.
const MAX_COMBINATIONS: usize = 32;

pub fn analyze_all(once: impl Fn() -> Analysis) -> Analysis {
    let actual = build::current();
    build::take_log();
    let mut main = once();
    let mut log = build::take_log();
    main.inactive = log.variants.iter()
        .filter(|v| !v.retained)
        .map(|v| InactiveRange { file: PathBuf::from(&v.file), start_line: v.span.line, end_line: v.end_line })
        .collect();
    if log.variants.is_empty() && log.warnings.is_empty() {
        return main;
    }

    let mut seen: HashSet<(PathBuf, usize, usize, String)> = main.diagnostics.iter().map(diag_key).collect();
    let mut done: BTreeSet<BTreeMap<String, String>> = BTreeSet::from([actual.values.clone()]);
    let mut used: BTreeMap<String, BTreeSet<String>> = std::mem::take(&mut log.used);
    let mut notes = log;
    loop {
        let pending: Vec<BuildConfig> = combinations(&used, &actual).into_iter()
            .filter(|c| !done.contains(&c.values))
            .collect();
        if pending.is_empty() || done.len() >= MAX_COMBINATIONS {
            break;
        }
        for config in pending.into_iter().take(MAX_COMBINATIONS - done.len()) {
            done.insert(config.values.clone());
            let other = build::with_config(config.clone(), &once);
            let mut log = build::take_log();
            for (name, values) in std::mem::take(&mut log.used) {
                used.entry(name).or_default().extend(values);
            }
            let names: BTreeSet<String> = used.keys().cloned().collect();
            for mut d in other.diagnostics {
                if seen.insert(diag_key(&d)) {
                    d.message = format!("{} [{}]", d.message, config.describe(&names));
                    main.diagnostics.push(d);
                }
            }
            if let (Some(checked), Some(o)) = (main.checked.as_mut(), other.checked) {
                checked.references.extend(o.references);
            }
            notes.variants.extend(log.variants);
            notes.warnings.extend(log.warnings);
        }
    }
    main.diagnostics.extend(selection_warnings(notes, &mut seen));
    main
}

fn diag_key(d: &Diagnostic) -> (PathBuf, usize, usize, String) {
    (d.file.clone(), d.line, d.col, d.message.clone())
}

fn warning(file: &str, line: usize, col: usize, message: String) -> Diagnostic {
    Diagnostic { file: PathBuf::from(file), line, col, message, severity: Severity::Warning, runtime_ctx: None }
}

/// Avertissements du parsing et variantes jamais retenues.
fn selection_warnings(log: SelectionLog, seen: &mut HashSet<(PathBuf, usize, usize, String)>) -> Vec<Diagnostic> {
    let retained: HashSet<(String, usize, usize)> = log.variants.iter()
        .filter(|v| v.retained)
        .map(|v| (v.file.clone(), v.span.line, v.span.col))
        .collect();
    let never = log.variants.iter()
        .filter(|v| !retained.contains(&(v.file.clone(), v.span.line, v.span.col)))
        .map(|v| warning(&v.file, v.span.line, v.span.col, "variante 'when' jamais retenue : une variante précédente l'emporte pour toutes les valeurs des constantes de build".into()));
    let parsed = log.warnings.into_iter().map(|(file, span, message)| warning(&file, span.line, span.col, message));
    parsed.chain(never).filter(|d| seen.insert(diag_key(d))).collect()
}

/// Combinaisons des constantes lues : pour chacune, les valeurs comparées,
/// la valeur courante et une valeur non citée (« autre »).
fn combinations(used: &BTreeMap<String, BTreeSet<String>>, actual: &BuildConfig) -> Vec<BuildConfig> {
    let mut out = vec![actual.clone()];
    for (name, mentioned) in used {
        let mut values: BTreeSet<String> = mentioned.clone();
        if let Some(v) = actual.get(name) {
            values.insert(v.to_string());
        }
        let domain = DOMAINS.iter().find(|(n, _)| n == name).map(|(_, d)| *d).unwrap_or(&[]);
        if let Some(other) = domain.iter().find(|d| !values.contains(**d)) {
            values.insert(other.to_string());
        }
        out = out.into_iter()
            .flat_map(|config| values.iter().map(move |v| {
                let mut c = config.clone();
                c.values.insert(name.clone(), v.clone());
                c
            }).collect::<Vec<_>>())
            .collect();
    }
    out
}
