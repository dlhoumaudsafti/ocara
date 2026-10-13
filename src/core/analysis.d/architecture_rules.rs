//! Règles de l'architecture hexagonale pour un fichier : namespace conforme
//! au chemin, imports (sens des couches, frontières entre contextes, rien
//! hors du répertoire hexagonal, bibliothèque externe depuis
//! l'infrastructure seulement) et cibles de `wiring` (même contexte ou
//! racine `shared`). Les fichiers hors du répertoire hexagonal ne sont pas
//! contrôlés.

use std::path::{Path, PathBuf};

use super::architecture::Config;
use super::imports::resolve_import_path;
use crate::core::diagnostics::Diagnostic;
use crate::parsing::ast::Program;
use crate::parsing::token::Span;

pub enum Place {
    /// Hors du dossier du projet : bibliothèque externe, importable depuis la
    /// couche infrastructure seulement.
    External,
    /// Dans le projet, hors du répertoire hexagonal (et des racines partagées).
    Outside,
    /// Dans le répertoire hexagonal, sans `<contexte>/<couche>` reconnu.
    Unclassified,
    Layer { context: String, layer: &'static str },
}

/// Couches dont `layer` peut dépendre (vers l'intérieur).
fn allowed(layer: &str) -> &'static [&'static str] {
    match layer {
        "domain" => &["domain"],
        "application" => &["application", "domain"],
        _ => &["infrastructure", "application", "domain"],
    }
}

/// Vérifie `file` ; renvoie les fichiers du projet qu'il importe (à
/// vérifier à leur tour).
pub fn check_file(config: &Config, file: &Path, program: &Program, entry: bool, out: &mut Vec<Diagnostic>) -> Vec<PathBuf> {
    let dir = if entry { config.root.clone() } else { file.parent().unwrap_or(&config.root).to_path_buf() };
    let resolve = |written: &str, relative: bool| -> Option<PathBuf> {
        let base = if relative { &dir } else { &config.root };
        resolve_import_path(written, base, &program.namespace, &config.root).canonicalize().ok()
    };
    let imports: Vec<(PathBuf, &Span)> = program.imports.iter()
        .filter(|i| i.path.first().is_some_and(|p| p != "ocara"))
        .filter_map(|i| match &i.file_path {
            Some(fp) => resolve(fp, true),
            None => resolve(&i.path.join("/"), false),
        }.map(|t| (t, &i.span)))
        .collect();
    let next = imports.iter().map(|(t, _)| t.clone()).collect();

    let in_src = file.starts_with(&config.src);
    if in_src {
        let expected = config.dotted(file.parent().unwrap_or(file));
        if program.namespace.as_deref() != Some(expected.as_str()) {
            out.push(Diagnostic::error(file, 1, 1, format!(
                "architecture hexagonale : le namespace de ce fichier doit refléter son chemin : 'namespace {}' (trouvé : {})",
                expected, program.namespace.as_deref().map(|n| format!("'namespace {}'", n)).unwrap_or_else(|| "aucun".into()))));
        }
    }
    let (context, layer) = match config.place(file) {
        Place::Outside | Place::External => return next,
        Place::Unclassified => {
            out.push(Diagnostic::error(file, 1, 1, unclassified(config, file)));
            return next;
        }
        Place::Layer { context, layer } => (context, layer),
    };
    let me = config.dotted(file);
    for (target, span) in &imports {
        if let Some(msg) = import_violation(config, &me, &context, layer, target) {
            out.push(Diagnostic::error(file, span.line, span.col, msg));
        }
    }
    for interface in &program.interfaces {
        for wiring in &interface.wirings {
            let Some(target) = resolve(&wiring.path.join("/"), false) else { continue };
            if let Some(msg) = wiring_violation(config, file, &context, &target) {
                out.push(Diagnostic::error(file, wiring.span.line, wiring.span.col, msg));
            }
        }
    }
    next
}

fn unclassified(config: &Config, file: &Path) -> String {
    let mut aliases: Vec<String> = config.layers.keys().cloned().collect();
    aliases.sort();
    format!("architecture hexagonale : '{}' n'est pas classable — attendu {}/<contexte>/<couche>/… (couches : {})",
        config.dotted(file), config.src_name.replace('.', "/"), aliases.join(", "))
}

fn import_violation(config: &Config, me: &str, context: &str, layer: &str, target: &Path) -> Option<String> {
    let them = config.dotted(target);
    match config.place(target) {
        // Bibliothèque externe : détail technique, réservé à l'infrastructure.
        Place::External => (layer != "infrastructure").then(|| format!(
            "architecture hexagonale : '{}' ({}) ne peut pas importer la bibliothèque externe '{}' — seule la couche infrastructure ({}) importe une bibliothèque hors du projet",
            me, layer, target.display(), config.alias("infrastructure"))),
        Place::Outside => Some(match config.in_other_src(target) {
            Some(other) => format!(
                "architecture hexagonale : '{}' importe '{}', situé dans un second répertoire hexagonal '{}' — un programme n'a qu'une seule racine hexagonale ('{}/')",
                me, them, other.strip_prefix(&config.root).unwrap_or(&other).display(), config.src_name.replace('.', "/")),
            None => format!(
                "architecture hexagonale : '{}' ({}) ne peut pas importer '{}', hors du répertoire hexagonal '{}/'",
                me, layer, them, config.src_name.replace('.', "/")),
        }),
        Place::Unclassified => Some(format!("architecture hexagonale : '{}' importe '{}', qui n'est pas classable", me, them)),
        Place::Layer { context: other, layer: to } => {
            if other != context && !config.is_shared(target) {
                return Some(format!(
                    "architecture hexagonale : les contextes '{}' et '{}' sont isolés — '{}' ne peut pas importer '{}' (déclarer la racine dans 'shared' pour la partager)",
                    context, other, me, them));
            }
            (!allowed(layer).contains(&to)).then(|| format!(
                "architecture hexagonale : dépendance de '{}' vers '{}' interdite — {} ({}) peut dépendre de {}, pas de {} ({})",
                me, them, layer, config.alias(layer), allowed(layer).join(" et "), to, config.alias(to)))
        }
    }
}

fn wiring_violation(config: &Config, file: &Path, context: &str, target: &Path) -> Option<String> {
    let them = config.dotted(target);
    match config.place(target) {
        Place::Layer { context: other, .. } if other == context || config.is_shared(target) || config.is_shared(file) => None,
        Place::Layer { context: other, .. } => Some(format!(
            "architecture hexagonale : 'wiring {}' vise le contexte '{}' — une interface du contexte '{}' ne peut viser que son contexte ou une racine 'shared'",
            them, other, context)),
        _ => Some(format!("architecture hexagonale : 'wiring {}' vise un fichier hors des contextes du répertoire hexagonal", them)),
    }
}
