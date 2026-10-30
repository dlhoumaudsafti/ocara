// Styles nommés (`styles:`) d'un `.oc.ui` et des fichiers d'écrans qu'il
// inclut (`screens.includes`) : un fichier inclus peut déclarer, à côté de
// ses écrans, les styles qu'ils utilisent. Tous partagent un seul espace de
// noms : un style est utilisable dans n'importe quel écran, et déclaré deux
// fois (même dans deux fichiers) c'est une erreur.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value as Json};

use super::schema;
use super::values::{is_ident, text};
use super::yaml::{self, Kind, Node};
use super::Checker;

impl Checker {
    /// Noms de styles du fichier `root` et de ses fichiers inclus, connus
    /// avant les nœuds : un écran peut utiliser un style déclaré dans un
    /// fichier inclus plus loin. Les erreurs (fichier absent, YAML invalide)
    /// sont signalées par la validation elle-même.
    pub(super) fn declare_styles(&mut self, root: &Node) {
        let mut seen = BTreeSet::new();
        collect_names(root, &self.file, &mut seen, &mut self.styles);
    }

    /// Styles d'un fichier (racine ou inclus).
    pub(super) fn styles(&mut self, node: &Node) -> Map<String, Json> {
        let entries = self.entries(node, "styles").unwrap_or_default();
        let mut out = Map::new();
        for (key, name, value) in entries {
            if !is_ident(name) {
                self.fail::<()>(key, format!("nom de style '{name}' invalide (lettres, chiffres, '_')"));
                continue;
            }
            if let Some((file, line)) = self.style_decls.get(name).cloned() {
                self.fail::<()>(key, format!("style '{name}' déjà déclaré ({}:{line})", file.display()));
                continue;
            }
            self.style_decls.insert(name.to_string(), (self.file.clone(), key.line));
            if let Some(style) = self.table(value, &format!("styles.{name}"), schema::STYLE_PROPS) {
                out.insert(name.to_string(), Json::Object(style));
            }
        }
        out
    }
}

/// Noms déclarés sous `styles:` de `root` (fichier `file`), puis dans les
/// fichiers de `screens.includes`, récursivement (chaque fichier une fois).
fn collect_names(root: &Node, file: &Path, seen: &mut BTreeSet<PathBuf>, names: &mut BTreeSet<String>) {
    seen.insert(std::fs::canonicalize(file).unwrap_or(file.to_path_buf()));
    let Kind::Map(pairs) = &root.kind else { return };
    for (key, value) in pairs {
        match (text(key), &value.kind) {
            (Some("styles"), Kind::Map(styles)) => {
                names.extend(styles.iter().filter_map(|(k, _)| text(k)).filter(|n| is_ident(n)).map(str::to_string));
            }
            (Some("screens"), Kind::Map(screens)) => {
                let includes = screens.iter().filter(|(k, _)| text(k) == Some("includes"));
                for (_, list) in includes {
                    let Kind::Seq(items) = &list.kind else { continue };
                    for relative in items.iter().filter_map(text) {
                        let path = file.parent().unwrap_or(Path::new("")).join(relative);
                        let canonical = std::fs::canonicalize(&path).unwrap_or(path.clone());
                        if seen.contains(&canonical) {
                            continue;
                        }
                        let Ok(source) = std::fs::read_to_string(&path) else { continue };
                        if let Ok(Some(included)) = yaml::load(&source) {
                            collect_names(&included, &path, seen, names);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
