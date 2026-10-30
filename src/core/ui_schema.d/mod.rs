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
mod styles;
mod variants;
pub use variants::check_interface;
mod values;
pub mod yaml;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

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
    /// Variables `${nom}` des textes, remplacées à l'exécution.
    pub vars: BTreeSet<String>,
    /// Fichiers lus : le `.oc.ui` et ses `includes`.
    pub files: Vec<PathBuf>,
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
        file: file.to_path_buf(),
        errors: Vec::new(),
        screen_names: HashMap::new(),
        files: vec![file.to_path_buf()],
        vars: BTreeSet::new(),
        ids: BTreeMap::new(),
        actions: BTreeSet::new(),
        icons: BTreeSet::new(),
        styles: BTreeSet::new(),
        style_decls: HashMap::new(),
        included_styles: Map::new(),
        theme_styles: Map::new(),
        in_item: 0,
    };
    checker.declare_styles(&root);
    let tree = checker.root(&root);
    if !checker.errors.is_empty() {
        checker.errors.sort_by(|a, b| (&a.file, a.line, a.col).cmp(&(&b.file, b.line, b.col)));
        return Err(checker.errors);
    }
    let ids: BTreeMap<String, String> = checker.ids.into_iter().map(|(id, (ty, _))| (id, ty)).collect();
    let mut tree = tree.unwrap_or_default();
    tree.insert("ids".into(), json!(ids));
    tree.insert("actions".into(), json!(checker.actions));
    let icons: Map<String, Json> = checker.icons.iter().filter_map(|name| Some((name.clone(), json!(icons::path(name)?)))).collect();
    tree.insert("icons".into(), Json::Object(icons));
    tree.insert("vars".into(), json!(checker.vars));
    Ok(UiSpec { tree: Json::Object(tree), ids, actions: checker.actions, vars: checker.vars, files: checker.files })
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
pub(super) struct Checker {
    /// Fichier en cours (le `.oc.ui`, ou un fichier inclus).
    file: PathBuf,
    errors: Vec<Diagnostic>,
    /// Écrans déclarés : fichier et ligne, pour signaler un doublon entre fichiers.
    screen_names: HashMap<String, (PathBuf, usize)>,
    /// Fichiers lus, dans l'ordre ; ceux en cours forment la pile des includes.
    files: Vec<PathBuf>,
    /// Variables `${nom}` rencontrées dans les textes.
    vars: BTreeSet<String>,
    /// `id` → (type du nœud, ligne de déclaration).
    ids: BTreeMap<String, (String, usize)>,
    actions: BTreeSet<String>,
    /// Icônes nommées, copiées dans l'arbre (`icons`).
    icons: BTreeSet<String>,
    /// Noms déclarés sous `styles:` (fichiers inclus compris), connus avant les nœuds.
    styles: BTreeSet<String>,
    /// Style → fichier et ligne de sa déclaration, pour signaler un doublon entre fichiers.
    style_decls: HashMap<String, (PathBuf, usize)>,
    /// Styles déclarés dans les fichiers inclus, ajoutés à ceux de la racine.
    included_styles: Map<String, Json>,
    /// Surcharges par thème (`styles.dark`, `styles.light`), tous fichiers confondus.
    theme_styles: Map<String, Json>,
    /// Profondeur dans un gabarit `item:` de liste (`bind` permis).
    in_item: usize,
}

type Entry<'n> = (&'n Node, &'n str, &'n Node);

impl Checker {
    /// Enregistre une erreur sur `node` ; rend toujours `None`.
    fn fail<T>(&mut self, node: &Node, message: String) -> Option<T> {
        self.errors.push(Diagnostic::error(&self.file, node.line, node.col, message));
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
        // Styles des fichiers inclus (lus avec leurs écrans) : avec ceux de la racine.
        if !self.included_styles.is_empty() {
            let included = std::mem::take(&mut self.included_styles);
            if let Some(Json::Object(styles)) = out.get_mut("styles") {
                styles.extend(included);
            } else {
                out.insert("styles".into(), Json::Object(included));
            }
        }
        if !self.theme_styles.is_empty() {
            out.insert("themeStyles".into(), Json::Object(std::mem::take(&mut self.theme_styles)));
        }
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

    /// Écrans dans l'ordre du fichier : (nom, écran, ligne). La clé
    /// `includes` liste d'autres `.oc.ui` dont les écrans s'ajoutent ici.
    fn screens(&mut self, node: &Node) -> Option<Vec<(String, Json, usize)>> {
        let entries = self.entries(node, "screens")?;
        let mut out = Vec::new();
        for (key, name, value) in entries {
            if name == "includes" {
                out.extend(self.includes(value));
                continue;
            }
            if !is_ident(name) {
                self.fail::<()>(key, format!("nom d'écran '{name}' invalide (lettres, chiffres, '_')"));
                continue;
            }
            if let Some((file, line)) = self.screen_names.get(name).cloned() {
                self.fail::<()>(key, format!("écran '{name}' déjà déclaré ({}:{line})", file.display()));
                continue;
            }
            self.screen_names.insert(name.to_string(), (self.file.clone(), key.line));
            let has_layout = matches!(&value.kind, Kind::Map(pairs) if pairs.iter().any(|(k, _)| text(k) == Some("layout")));
            let Some(screen) = self.table(value, &format!("screens.{name}"), schema::SCREEN_PROPS) else { continue };
            if !has_layout {
                self.fail::<()>(value, format!("écran '{name}' sans 'layout'"));
            }
            out.push((name.to_string(), Json::Object(screen), key.line));
        }
        if out.is_empty() && self.errors.is_empty() {
            return self.fail(node, "'screens' : au moins un écran attendu".to_string());
        }
        Some(out)
    }

    /// `screens.includes` : fichiers `.oc.ui` (relatifs au fichier courant)
    /// qui ne contiennent que `screens:` et `styles:`, includes imbriqués compris.
    fn includes(&mut self, node: &Node) -> Vec<(String, Json, usize)> {
        let Kind::Seq(items) = &node.kind else {
            self.fail::<()>(node, "'screens.includes' : liste de fichiers .oc.ui attendue".to_string());
            return Vec::new();
        };
        let mut out = Vec::new();
        for item in items {
            let Some(relative) = text(item) else {
                self.fail::<()>(item, "'screens.includes' : chemin de fichier attendu".to_string());
                continue;
            };
            let path = self.file.parent().unwrap_or(Path::new("")).join(relative);
            let canonical = std::fs::canonicalize(&path).unwrap_or(path.clone());
            if self.files.iter().any(|f| std::fs::canonicalize(f).unwrap_or(f.clone()) == canonical) {
                self.fail::<()>(item, format!("'{relative}' déjà inclus (inclusion circulaire ou en double)"));
                continue;
            }
            let source = match std::fs::read_to_string(&path) {
                Ok(source) => source,
                Err(e) => {
                    self.fail::<()>(item, format!("lecture de '{}' : {e}", path.display()));
                    continue;
                }
            };
            self.files.push(path.clone());
            let outer = std::mem::replace(&mut self.file, path);
            out.extend(self.included_screens(&source));
            self.file = outer;
        }
        out
    }

    /// Écrans d'un fichier inclus (fichier courant : celui-ci) ; ses styles
    /// vont dans `included_styles`.
    fn included_screens(&mut self, source: &str) -> Vec<(String, Json, usize)> {
        let root = match yaml::load(source) {
            Ok(Some(root)) => root,
            Ok(None) => {
                self.errors.push(Diagnostic::error(&self.file, 1, 1, "fichier inclus vide (attendu : screens)"));
                return Vec::new();
            }
            Err((line, col, message)) => {
                self.errors.push(Diagnostic::error(&self.file, line, col, message + &comment_hint(source, line)));
                return Vec::new();
            }
        };
        let Some(entries) = self.entries(&root, "racine") else { return Vec::new() };
        let mut out = Vec::new();
        for (key, name, value) in entries {
            match name {
                "screens" => out.extend(self.screens(value).unwrap_or_default()),
                "styles" => {
                    let styles = self.styles(value);
                    self.included_styles.extend(styles);
                }
                _ => {
                    self.fail::<()>(key, format!("clé '{name}' inconnue dans un fichier inclus (seules 'screens' et 'styles' sont permises)"));
                }
            }
        }
        out
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
