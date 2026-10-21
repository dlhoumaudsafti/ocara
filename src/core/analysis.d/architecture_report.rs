//! Rapport d'architecture d'un projet (`ocara main.oc --architecture-report`),
//! lu par l'encart Architecture de l'extension VS Code : type d'architecture
//! (configuration effective du point d'entrée, voir `architecture::effective`),
//! répertoire hexagonal, alias des couches, racines partagées, et pour chaque
//! script `.oc` du projet sa place (contexte et couche, hors du répertoire
//! hexagonal, non classé) et ses violations — chaque script vérifié pour ses
//! propres imports et `wiring` (`architecture_rules::check_file`).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::architecture::{effective, Config, Place};
use super::architecture_rules::check_file;
use super::imports::parse_module;
use crate::core::diagnostics::Diagnostic;

/// Dossiers jamais parcourus (caches, sorties de compilation, dépendances).
const SKIPPED_DIRS: [&str; 4] = [".ocaraunit_cache", "target", "node_modules", ".git"];

pub fn report(input: &Path, root: &Path) -> Value {
    let own = parse_module(input).ok().and_then(|p| p.architecture);
    match effective(own.as_ref(), input, root) {
        Ok(None) => json!({ "mode": "permissive" }),
        Err(diags) => json!({ "mode": "hexagonal", "configErrors": diags.iter().map(error_json).collect::<Vec<_>>() }),
        Ok(Some(config)) => hexagonal(&config, input),
    }
}

fn hexagonal(config: &Config, input: &Path) -> Value {
    let entry = input.canonicalize().unwrap_or_else(|_| input.to_path_buf());
    let mut files = Vec::new();
    collect(&config.root, &mut files);
    files.sort();
    let scripts: Vec<Value> = files.iter().map(|file| {
        let mut errors = Vec::new();
        if let Ok(program) = parse_module(file) {
            check_file(config, file, &program, *file == entry, &mut errors);
        }
        let own: Vec<Value> = errors.iter().filter(|d| same_file(&d.file, file)).map(error_json).collect();
        let place = match config.place(file) {
            Place::Layer { context, layer } => json!({ "kind": "layer", "context": context, "layer": layer }),
            Place::Outside => json!({ "kind": "outside" }),
            Place::Unclassified => json!({ "kind": "unclassified" }),
            Place::External => json!({ "kind": "external" }),
        };
        json!({ "path": file.to_string_lossy(), "place": place, "errors": own })
    }).collect();
    let layers: serde_json::Map<String, Value> = ["application", "infrastructure", "domain"].iter()
        .map(|l| (l.to_string(), Value::String(config.alias(l))))
        .collect();
    json!({
        "mode": "hexagonal",
        "src": config.src_name,
        "layers": layers,
        "shared": config.shared.iter().map(|s| config.dotted(s)).collect::<Vec<_>>(),
        "files": scripts,
    })
}

/// Scripts `.oc` du projet, fichiers générés par ocaraunit exclus.
fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !SKIPPED_DIRS.contains(&name.as_str()) {
                collect(&path, out);
            }
        } else if name.ends_with(".oc") && !name.starts_with("__ocaraunit__") {
            out.push(path.canonicalize().unwrap_or(path));
        }
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    a.canonicalize().map(|a| a == b).unwrap_or(a == b)
}

fn error_json(d: &Diagnostic) -> Value {
    json!({ "file": d.file.to_string_lossy(), "line": d.line, "col": d.col, "message": d.message })
}
