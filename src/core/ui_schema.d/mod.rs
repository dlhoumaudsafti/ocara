// ─────────────────────────────────────────────────────────────────────────────
// Schéma des fichiers `.oc.ui` (interface décrite en YAML, builtin `ocara.UI`,
// docs/roadmap.d/langage-builtin-ui-multiplateforme.md §4).
//
// Le fichier est lu à la compilation : toute erreur (YAML invalide, type de
// nœud, propriété ou valeur inconnue, `id` en double, style non déclaré…)
// est une erreur de compilation située à la ligne du `.oc.ui`. Le résultat
// est un arbre JSON normalisé, embarqué tel quel dans le binaire et lu par
// les moteurs de rendu (Tauri, Compose) :
//
//   { "window": {…}, "styles": {…}, "start": "home",
//     "screens": { "home": { "title": …, "layout": <nœud> } },
//     "menu": […], "ids": { "search": "input" }, "actions": ["search"],
//     "icons": { "search": "<tracé SVG 24×24>" } }
//
// Un `layout:` à la racine (sans `screens:`) est un écran unique `main`.
// ─────────────────────────────────────────────────────────────────────────────

mod icons;
mod images;
pub mod json_schema;
mod nodes;
pub mod preview;
pub mod schema;
mod values;
pub mod yaml;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{json, Map, Value as Json};

use crate::core::diagnostics::Diagnostic;
use schema::Ty;
use values::{is_ident, text};
pub use values::suggest;
use yaml::{Kind, Node};

/// `.oc.ui` validé.
#[derive(Debug, Clone)]
pub struct UiSpec {
    /// Arbre normalisé (voir l'en-tête).
    pub tree: Json,
    /// `id` déclarés et type de leur nœud.
    pub ids: BTreeMap<String, String>,
    /// Actions nommées par les événements et les menus.
    pub actions: BTreeSet<String>,
}

/// Lit et valide le fichier `path`.
pub fn check_file(path: &Path) -> Result<UiSpec, Vec<Diagnostic>> {
    match std::fs::read_to_string(path) {
        Ok(source) => check_source(path, &source),
        Err(e) => Err(vec![Diagnostic::error(path, 1, 1, format!("lecture de '{}' : {e}", path.display()))]),
    }
}

/// Valide `source`, contenu du fichier `file` (pour situer les erreurs).
pub fn check_source(file: &Path, source: &str) -> Result<UiSpec, Vec<Diagnostic>> {
    let root = match yaml::load(source) {
        Ok(Some(root)) => root,
        Ok(None) => return Err(vec![Diagnostic::error(file, 1, 1, "fichier .oc.ui vide (attendu : window, screens…)")]),
        Err((line, col, message)) => return Err(vec![Diagnostic::error(file, line, col, message + &comment_hint(source, line))]),
    };
    let mut checker = Checker {
        file,
        errors: Vec::new(),
        ids: BTreeMap::new(),
        actions: BTreeSet::new(),
        icons: BTreeSet::new(),
        styles: BTreeSet::new(),
        in_item: 0,
    };
    let tree = checker.root(&root);
    if !checker.errors.is_empty() {
        checker.errors.sort_by_key(|d| (d.line, d.col));
        return Err(checker.errors);
    }
    let ids: BTreeMap<String, String> = checker.ids.into_iter().map(|(id, (ty, _))| (id, ty)).collect();
    let mut tree = tree.unwrap_or_default();
    tree.insert("ids".into(), json!(ids));
    tree.insert("actions".into(), json!(checker.actions));
    let icons: Map<String, Json> = checker.icons.iter().filter_map(|name| Some((name.clone(), json!(icons::path(name)?)))).collect();
    tree.insert("icons".into(), Json::Object(icons));
    Ok(UiSpec { tree: Json::Object(tree), ids, actions: checker.actions })
}

/// Erreur YAML causée le plus souvent par une couleur sans guillemets dans
/// une table `{ … }` : `color: #fff }` fait du reste de la ligne un
/// commentaire, accolade fermante comprise.
fn comment_hint(source: &str, line: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let suspect = (line.saturating_sub(2)..line.min(lines.len())).any(|i| lines[i].contains(": #"));
    if suspect { " — '#' précédé d'une espace commence un commentaire YAML : écrire la couleur entre guillemets (\"#rrggbb\")".to_string() } else { String::new() }
}

/// État de la validation d'un fichier.
pub(super) struct Checker<'a> {
    file: &'a Path,
    errors: Vec<Diagnostic>,
    /// `id` → (type du nœud, ligne de déclaration).
    ids: BTreeMap<String, (String, usize)>,
    actions: BTreeSet<String>,
    /// Icônes nommées, copiées dans l'arbre (`icons`).
    icons: BTreeSet<String>,
    /// Noms déclarés sous `styles:`, connus avant les nœuds.
    styles: BTreeSet<String>,
    /// Profondeur dans un gabarit `item:` de liste (`bind` permis).
    in_item: usize,
}

type Entry<'n> = (&'n Node, &'n str, &'n Node);

impl Checker<'_> {
    /// Enregistre une erreur sur `node` ; rend toujours `None`.
    fn fail<T>(&mut self, node: &Node, message: String) -> Option<T> {
        self.errors.push(Diagnostic::error(self.file, node.line, node.col, message));
        None
    }

    /// Paires d'une table, clés en texte ; signale les clés en double.
    fn entries<'n>(&mut self, node: &'n Node, what: &str) -> Option<Vec<Entry<'n>>> {
        let Kind::Map(pairs) = &node.kind else {
            return self.fail(node, format!("'{what}' : table attendue (clé: valeur)"));
        };
        let mut out: Vec<Entry<'n>> = Vec::new();
        for (key, value) in pairs {
            let Some(name) = text(key) else {
                self.fail::<()>(key, format!("'{what}' : clé texte attendue"));
                continue;
            };
            if let Some((first, ..)) = out.iter().find(|(_, n, _)| *n == name) {
                let line = first.line;
                self.fail::<()>(key, format!("clé '{name}' en double dans '{what}' (première ligne {line})"));
                continue;
            }
            out.push((key, name, value));
        }
        Some(out)
    }

    fn root(&mut self, root: &Node) -> Option<Map<String, Json>> {
        let entries = self.entries(root, "racine")?;
        let get = |name: &str| entries.iter().find(|(_, n, _)| *n == name).map(|&(k, _, v)| (k, v));
        for &(key, name, _) in &entries {
            if !schema::ROOT_KEYS.contains(&name) {
                self.fail::<()>(key, format!("clé '{name}' inconnue à la racine (clés : {}){}",
                    schema::ROOT_KEYS.join(", "), suggest(name, schema::ROOT_KEYS)));
            }
        }
        let mut out = Map::new();
        // Styles d'abord : les nœuds y font référence.
        if let Some((_, styles)) = get("styles") {
            out.insert("styles".into(), Json::Object(self.styles(styles)));
        }
        if let Some((_, window)) = get("window") {
            out.insert("window".into(), Json::Object(self.window(window)));
        }
        let screens = match (get("screens"), get("layout")) {
            (Some(_), Some((key, _))) => self.fail(key, "'layout' à la racine et 'screens' s'excluent (un écran : 'layout', plusieurs : 'screens')".to_string()),
            (Some((_, screens)), None) => self.screens(screens),
            (None, Some((_, layout))) => self.node(layout).map(|node| vec![("main".to_string(), json!({ "layout": node }), root.line)]),
            (None, None) => self.fail(root, "aucun écran : 'layout' (un écran) ou 'screens' (plusieurs) attendu".to_string()),
        };
        if let Some(screens) = screens {
            self.start(root, get("start").map(|(_, v)| v), &screens, &mut out);
            let screens: Map<String, Json> = screens.into_iter().map(|(name, screen, _)| (name, screen)).collect();
            out.insert("screens".into(), Json::Object(screens));
        }
        if let Some((_, menu)) = get("menu") {
            if let Some(menu) = self.value(menu, Ty::Menu, "'menu'") {
                out.insert("menu".into(), menu);
            }
        }
        Some(out)
    }

    fn styles(&mut self, node: &Node) -> Map<String, Json> {
        let entries = self.entries(node, "styles").unwrap_or_default();
        for &(key, name, _) in &entries {
            if is_ident(name) {
                self.styles.insert(name.to_string());
            } else {
                self.fail::<()>(key, format!("nom de style '{name}' invalide (lettres, chiffres, '_')"));
            }
        }
        let mut out = Map::new();
        for (_, name, value) in entries {
            if let Some(style) = self.table(value, &format!("styles.{name}"), schema::STYLE_PROPS) {
                out.insert(name.to_string(), Json::Object(style));
            }
        }
        out
    }

    fn window(&mut self, node: &Node) -> Map<String, Json> {
        let plain: Vec<(&str, Ty)> = schema::WINDOW_PROPS.iter().copied().filter(|(n, _)| !schema::PLATFORMS.contains(n)).collect();
        let mut out = Map::new();
        for (key, name, value) in self.entries(node, "window").unwrap_or_default() {
            let platform = match name {
                "tauri" => Some(schema::WINDOW_TAURI),
                "android" => Some(schema::WINDOW_ANDROID),
                _ => None,
            };
            let checked = match (platform, plain.iter().find(|(n, _)| *n == name)) {
                (Some(props), _) => self.table(value, &format!("window.{name}"), props).map(Json::Object),
                (None, Some(&(_, ty))) => self.value(value, ty, &format!("'window.{name}'")),
                (None, None) => {
                    let known: Vec<&str> = schema::WINDOW_PROPS.iter().map(|(n, _)| *n).collect();
                    self.fail(key, format!("clé '{name}' inconnue dans 'window'{}", suggest(name, &known)))
                }
            };
            if let Some(v) = checked {
                out.insert(name.to_string(), v);
            }
        }
        out
    }

    /// Écrans dans l'ordre du fichier : (nom, écran, ligne).
    fn screens(&mut self, node: &Node) -> Option<Vec<(String, Json, usize)>> {
        let entries = self.entries(node, "screens")?;
        if entries.is_empty() {
            return self.fail(node, "'screens' : au moins un écran attendu".to_string());
        }
        let mut out = Vec::new();
        for (key, name, value) in entries {
            if !is_ident(name) {
                self.fail::<()>(key, format!("nom d'écran '{name}' invalide (lettres, chiffres, '_')"));
                continue;
            }
            let has_layout = matches!(&value.kind, Kind::Map(pairs) if pairs.iter().any(|(k, _)| text(k) == Some("layout")));
            let Some(screen) = self.table(value, &format!("screens.{name}"), schema::SCREEN_PROPS) else { continue };
            if !has_layout {
                self.fail::<()>(value, format!("écran '{name}' sans 'layout'"));
            }
            out.push((name.to_string(), Json::Object(screen), key.line));
        }
        Some(out)
    }

    /// `start` : écran affiché au lancement (le seul écran par défaut).
    fn start(&mut self, root: &Node, start: Option<&Node>, screens: &[(String, Json, usize)], out: &mut Map<String, Json>) {
        let names: Vec<&str> = screens.iter().map(|(n, ..)| n.as_str()).collect();
        let chosen = match start {
            Some(node) => match text(node) {
                Some(name) if names.contains(&name) => Some(name.to_string()),
                Some(name) => self.fail(node, format!("'start' : écran '{name}' non déclaré (écrans : {}){}", names.join(", "), suggest(name, &names))),
                None => self.fail(node, "'start' : nom d'écran attendu".to_string()),
            },
            None if names.len() == 1 => Some(names[0].to_string()),
            None if names.is_empty() => None,
            None => self.fail(root, format!("plusieurs écrans : 'start' doit nommer le premier affiché ({})", names.join(", "))),
        };
        if let Some(chosen) = chosen {
            out.insert("start".into(), Json::String(chosen));
        }
    }
}
