//! Couches de l'architecture hexagonale vues par le serveur de langage :
//! configuration effective du projet (core::analysis::architecture),
//! contexte et couche d'un fichier, segment de couche d'un chemin pointé
//! (`namespace`, `import`, `wiring`) pour les jetons sémantiques.

use std::collections::HashSet;
use std::path::Path;

use super::project;
use super::workspace::Workspace;
use crate::core::analysis::architecture::{effective, Config, Place};
use crate::parsing::token::Token;

/// Configuration hexagonale de `path` (`None` : permissive ou invalide).
pub fn config(ws: &Workspace, path: &Path) -> Option<Config> {
    let program = ws.text(path).and_then(|t| project::parse(&t))?;
    effective(program.architecture.as_ref(), path, &ws.project_root(path)).ok().flatten()
}

/// « car · infrastructure (infra) », « hors src », « bibliothèque externe »,
/// ou `None` si non classable.
pub fn describe(config: &Config, file: &Path) -> Option<(String, Option<String>, Option<&'static str>)> {
    match config.place(file) {
        Place::Layer { context, layer } => Some((format!("{} · {} ({})", context, layer, config.alias(layer)), Some(context), Some(layer))),
        Place::Outside => Some((format!("hors {}/", config.src_name.replace('.', "/")), None, None)),
        Place::External => Some(("bibliothèque externe".to_string(), None, None)),
        Place::Unclassified => None,
    }
}

/// Forme syntaxique des chemins : préfixe du répertoire hexagonal, racines
/// partagées hors de celui-ci, alias de couche.
pub struct LayerSyntax {
    src: Vec<String>,
    shared: Vec<Vec<String>>,
    aliases: HashSet<String>,
}

impl LayerSyntax {
    pub fn new(src: Vec<String>, shared: Vec<Vec<String>>, aliases: HashSet<String>) -> Self {
        Self { src, shared, aliases }
    }

    pub fn from_config(config: &Config) -> Self {
        let shared = config.shared.iter()
            .filter(|s| !s.starts_with(&config.src))
            .filter_map(|s| s.strip_prefix(&config.root).ok())
            .map(|rel| rel.iter().map(|c| c.to_string_lossy().into_owned()).collect())
            .collect();
        Self::new(config.src_name.split('.').map(str::to_string).collect(), shared, config.layers.keys().cloned().collect())
    }

    /// Segment de couche de `path` : `<src>.<contexte>.<couche>…`, ou
    /// `<racine partagée>.<couche>…`.
    pub fn layer_segment<'t>(&self, path: &[&'t Token]) -> Option<&'t Token> {
        let names: Vec<&str> = path.iter().map(|t| t.lexeme.as_str()).collect();
        let prefixed = |prefix: &[String]| names.len() > prefix.len() && names.iter().zip(prefix).all(|(a, b)| a == b);
        let index = if prefixed(&self.src) {
            self.src.len() + 1
        } else {
            self.shared.iter().find(|s| prefixed(s))?.len()
        };
        path.get(index).copied().filter(|t| self.aliases.contains(&t.lexeme))
    }
}
