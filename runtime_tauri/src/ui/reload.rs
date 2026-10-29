// Rechargement à chaud d'une interface (build de debug) : l'arbre porte
// `source` (chemin du `.oc.ui` et du compilateur, src/core/ui_file.rs).
// Pendant `run()`, le fichier est surveillé ; à chaque modification, le
// compilateur le revalide (`ocara x.oc.ui --dump`) et le moteur réaffiche
// le nouvel arbre en gardant l'état, les gestionnaires et l'écran courant.
// Une erreur est affichée sur la sortie d'erreur, l'interface reste telle quelle.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde_json::Value;

use ocara_runtime_ui as core;

use crate::window::live;

const PERIOD: Duration = Duration::from_millis(400);

/// Dernière modification parmi les fichiers de l'interface (includes et
/// variantes de taille compris).
fn modified(files: &[PathBuf]) -> Option<SystemTime> {
    files.iter().filter_map(|f| std::fs::metadata(f).and_then(|m| m.modified()).ok()).max()
}

/// Lance la surveillance si l'arbre de `this` le permet.
pub(crate) fn watch(this: i64) {
    let Some(source) = core::tree(this).and_then(|t| t.get("source").cloned()) else { return };
    let text = |key: &str| source.get(key).and_then(Value::as_str).map(str::to_string);
    let (Some(path), Some(compiler)) = (text("path"), text("compiler")) else { return };
    let path = PathBuf::from(path);
    let mut files: Vec<PathBuf> = source.get("files").and_then(Value::as_array).into_iter().flatten()
        .filter_map(Value::as_str).map(PathBuf::from).collect();
    files.push(path.clone());
    if !std::path::Path::new(&compiler).exists() {
        return;
    }
    std::thread::spawn(move || {
        let mut last = modified(&files);
        // La fenêtre peut ne pas être encore construite au démarrage du thread.
        let mut seen = false;
        loop {
            std::thread::sleep(PERIOD);
            match (live(this).is_some(), seen) {
                (true, _) => seen = true,
                (false, true) => return,
                (false, false) => continue,
            }
            let now = modified(&files);
            if now == last {
                continue;
            }
            last = now;
            reload(this, &compiler, &path);
        }
    });
}

fn reload(this: i64, compiler: &str, path: &PathBuf) {
    let output = match std::process::Command::new(compiler).arg(path).arg("--dump").output() {
        Ok(output) => output,
        Err(e) => return eprintln!("UI : rechargement impossible ({compiler}) : {e}"),
    };
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        return;
    }
    let Ok(mut tree) = serde_json::from_slice::<Value>(&output.stdout) else { return };
    if let Some(source) = core::tree(this).and_then(|t| t.get("source").cloned()) {
        tree["source"] = source;
    }
    let shown = core::reload(this, tree);
    super::show_tree(this, &shown);
    eprintln!("UI : {} rechargé", path.display());
}
