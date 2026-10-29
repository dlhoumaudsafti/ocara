// Variantes de taille d'une interface : à côté de `garage.oc.ui` (défaut),
// `garage.sm.oc.ui`, `garage.md.oc.ui`… pour les fenêtres ou écrans plus
// larges. La largeur disponible (pixels logiques : px CSS sur desktop, dp
// sur Android) donne une classe ; le moteur de rendu affiche la plus grande
// variante dont le seuil est atteint, sinon le défaut. Seuils calés sur les
// classes de taille de Material 3 (600, 840, 1200, 1600 dp).
//
// Toutes les variantes sont validées et embarquées à la compilation
// (`variants`, `sizes` dans l'arbre) : le choix, puis le changement à la
// volée (fenêtre redimensionnée, rotation, écran plié), se font à
// l'exécution, état gardé. Un même `id` a le même type dans toutes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value as Json};

use crate::core::diagnostics::Diagnostic;

use super::{check_file, UiSpec};

/// Classes et largeur minimale de chacune.
pub const SIZE_CLASSES: &[(&str, u32)] = &[
    ("xxs", 0),
    ("xs", 360),
    ("sm", 600),
    ("md", 840),
    ("lg", 1200),
    ("xl", 1600),
    ("xxl", 1920),
];

/// `garage.md.oc.ui` pour `garage.oc.ui` et la classe `md`.
fn variant_path(base: &Path, class: &str) -> Option<PathBuf> {
    let name = base.file_name()?.to_str()?;
    let stem = name.strip_suffix(".oc.ui")?;
    Some(base.with_file_name(format!("{stem}.{class}.oc.ui")))
}

/// Interface de `path` et ses variantes de taille présentes.
pub fn check_interface(path: &Path) -> Result<UiSpec, Vec<Diagnostic>> {
    let mut errors = Vec::new();
    let mut base = match check_file(path) {
        Ok(spec) => Some(spec),
        Err(e) => {
            errors.extend(e);
            None
        }
    };
    let mut variants: BTreeMap<&str, (PathBuf, UiSpec)> = BTreeMap::new();
    for &(class, _) in SIZE_CLASSES {
        let Some(file) = variant_path(path, class).filter(|f| f.exists()) else { continue };
        match check_file(&file) {
            Ok(spec) => {
                variants.insert(class, (file, spec));
            }
            Err(e) => errors.extend(e),
        }
    }
    let Some(spec) = base.as_mut() else { return Err(errors) };
    // Seuils toujours présents : `ui.sizeClass()` sans variante aussi.
    let sizes: Map<String, Json> = SIZE_CLASSES.iter().map(|(c, w)| (c.to_string(), json!(w))).collect();
    spec.tree["sizes"] = Json::Object(sizes);
    if variants.is_empty() {
        return if errors.is_empty() { Ok(base.unwrap()) } else { Err(errors) };
    }
    let mut trees = Map::new();
    for &(class, _) in SIZE_CLASSES {
        let Some((file, variant)) = variants.remove(class) else { continue };
        for (id, ty) in &variant.ids {
            match spec.ids.get(id) {
                Some(known) if known != ty => errors.push(Diagnostic::error(&file, 1, 1, format!(
                    "id '{id}' : nœud '{ty}' ici, '{known}' dans une autre variante (même type attendu)"))),
                Some(_) => {}
                None => {
                    spec.ids.insert(id.clone(), ty.clone());
                }
            }
        }
        spec.actions.extend(variant.actions.iter().cloned());
        spec.vars.extend(variant.vars.iter().cloned());
        spec.files.extend(variant.files.iter().cloned());
        trees.insert(class.to_string(), variant.tree);
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let tree = spec.tree.as_object_mut().expect("arbre : objet");
    tree.insert("ids".into(), json!(spec.ids));
    tree.insert("actions".into(), json!(spec.actions));
    tree.insert("vars".into(), json!(spec.vars));
    tree.insert("variants".into(), Json::Object(trees));
    Ok(base.unwrap())
}
