// ─────────────────────────────────────────────────────────────────────────────
// `use UI("x.oc.ui")` : le fichier est lu et validé à la compilation
// (src/core/ui_schema.d) et l'argument devient l'arbre JSON normalisé, en
// littéral : le binaire n'a rien à lire à l'exécution (APK compris). Une
// erreur du `.oc.ui` est une erreur de compilation à sa ligne.
//
// Appelé par le parcours de `core::render_file` (même suivi des constantes
// pour le chemin). Un chemin relatif part du dossier du fichier `.oc` qui
// contient le `use`. En build de debug, l'arbre porte aussi `source` (chemin
// du `.oc.ui` et du compilateur) pour le rechargement à chaud.
//
// Une variable initialisée par `use UI(…)` garde son arbre dans le suivi des
// constantes (`ui_key`) : `ui.on`, `ui.get`/`set`, `ui.navigate`… avec un
// littéral sont vérifiés contre le `.oc.ui` dès la compilation (ailleurs,
// dans un gestionnaire qui reçoit `ui:UI`, à l'exécution : UIException).
// ─────────────────────────────────────────────────────────────────────────────

use std::path::{Path, PathBuf};

use crate::parsing::ast::{Expr, Literal};
use crate::parsing::token::Span;

pub type DesugarError = (Span, String);

fn resolve(raw: &str, span: &Span) -> PathBuf {
    let path = Path::new(raw);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let dir = span.file.as_deref().and_then(|f| Path::new(f).parent()).map(Path::to_path_buf).unwrap_or_default();
    dir.join(path)
}

/// Remplace le chemin `raw` (argument de `use UI`) par l'arbre JSON.
pub fn desugar_new(args: &mut Vec<Expr>, span: &Span, raw: Option<String>) -> Result<(), DesugarError> {
    if args.is_empty() || args.len() > 2 {
        return Err((span.clone(), format!("use UI(path, vars) : 1 ou 2 arguments attendus, {} reçu(s)", args.len())));
    }
    // `use UI(path)` : pas de variables (`UI_init` reçoit toujours les deux).
    if args.len() == 1 {
        args.push(Expr::Map { entries: Vec::new(), span: span.clone() });
    }
    let raw = raw.ok_or_else(|| (span.clone(),
        "use UI(path) : le chemin du .oc.ui doit être connu à la compilation (littéral ou constante)".to_string()))?;
    let path = resolve(&raw, span);
    let spec = crate::core::ui_schema::check_interface(&path).map_err(|errors| {
        let first = &errors[0];
        let mut at = Span::new(first.line, first.col);
        at.file = Some(first.file.display().to_string());
        let more = match errors.len() {
            1 => String::new(),
            n => format!(" (et {} autre(s) erreur(s) : `ocara {}`)", n - 1, path.display()),
        };
        (at, format!("{}{more}", first.message))
    })?;
    let mut tree = spec.tree;
    // Build de debug : de quoi recharger l'interface à chaud quand le
    // `.oc.ui` change (runtime_tauri/src/ui/reload.rs).
    if crate::parsing::build::current().get("BUILD").unwrap_or("debug") == "debug" {
        let absolute = std::fs::canonicalize(&path).unwrap_or(path.clone());
        let compiler = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
        let files: Vec<String> = spec.files.iter()
            .map(|f| std::fs::canonicalize(f).unwrap_or(f.clone()).display().to_string())
            .collect();
        tree["source"] = serde_json::json!({ "path": absolute.display().to_string(), "compiler": compiler, "files": files });
    }
    args[0] = Expr::Literal(Literal::String(tree.to_string()), span.clone());
    Ok(())
}

/// Clé du suivi des constantes (core::render_file) qui porte l'arbre de la
/// variable `name` (`\0` : jamais un nom de variable).
pub fn ui_key(name: &str) -> String {
    format!("\0ui:{name}")
}

/// Arbre d'une expression `use UI(…)` déjà réécrite.
pub fn tree_of(value: &Expr) -> Option<String> {
    match value {
        Expr::New { class, args, .. } if class == "UI" => match args.first() {
            Some(Expr::Literal(Literal::String(json), _)) => Some(json.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Vérifie `ui.<method>("littéral", …)` contre l'arbre `tree`.
pub fn check_call(tree: &str, method: &str, args: &[Expr], span: &Span) -> Result<(), DesugarError> {
    let Some(Expr::Literal(Literal::String(name), _)) = args.first() else { return Ok(()) };
    let Ok(tree) = serde_json::from_str::<serde_json::Value>(tree) else { return Ok(()) };
    let ids = tree["ids"].as_object().cloned().unwrap_or_default();
    let id_of = |kind: Option<&str>| -> Result<(), DesugarError> {
        let found = ids.get(name).and_then(|t| t.as_str());
        let candidates: Vec<&str> = ids.iter()
            .filter(|(_, t)| kind.is_none_or(|k| t.as_str() == Some(k)))
            .map(|(id, _)| id.as_str())
            .collect();
        match (found, kind) {
            (None, _) => Err((span.clone(), format!("ui.{method} : id '{name}' absent du .oc.ui{}", suggestion(name, &candidates)))),
            (Some(t), Some(k)) if t != k => Err((span.clone(), format!("ui.{method} : '{name}' est un nœud '{t}', '{k}' attendu"))),
            _ => Ok(()),
        }
    };
    match method {
        "on" => {
            let mut actions: Vec<&str> = tree["actions"].as_array().into_iter().flatten().filter_map(|a| a.as_str()).collect();
            actions.extend(["back", "close", "ready", "resize"]);
            if actions.contains(&name.as_str()) { Ok(()) } else {
                Err((span.clone(), format!("ui.on : action '{name}' absente du .oc.ui{}", suggestion(name, &actions))))
            }
        }
        "get" | "set" | "has" => id_of(None),
        "setItems" => id_of(Some("list")),
        "postMessage" | "eval" | "webBack" | "webForward" | "reload" => id_of(Some("webview")),
        "navigate" => {
            let screens: Vec<&str> = tree["screens"].as_object().map(|s| s.keys().map(String::as_str).collect()).unwrap_or_default();
            if screens.contains(&name.as_str()) { Ok(()) } else {
                Err((span.clone(), format!("ui.navigate : écran '{name}' absent du .oc.ui{}", suggestion(name, &screens))))
            }
        }
        _ => Ok(()),
    }
}

fn suggestion(word: &str, candidates: &[&str]) -> String {
    match crate::core::ui_schema::suggest(word, candidates) {
        hint if !hint.is_empty() => hint,
        _ if candidates.is_empty() => String::new(),
        _ => format!(" (déclarés : {})", candidates.join(", ")),
    }
}
