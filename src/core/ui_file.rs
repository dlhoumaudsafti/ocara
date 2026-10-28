// ─────────────────────────────────────────────────────────────────────────────
// `use UI("x.oc.ui")` : le fichier est lu et validé à la compilation
// (src/core/ui_schema.d) et l'argument devient l'arbre JSON normalisé, en
// littéral : le binaire n'a rien à lire à l'exécution (APK compris). Une
// erreur du `.oc.ui` est une erreur de compilation à sa ligne.
//
// Appelé par le parcours de `core::render_file` (même suivi des constantes
// pour le chemin). Un chemin relatif part du dossier du fichier `.oc` qui
// contient le `use`.
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
    if args.len() != 1 {
        return Err((span.clone(), format!("use UI(path) : 1 argument attendu, {} reçu(s)", args.len())));
    }
    let raw = raw.ok_or_else(|| (span.clone(),
        "use UI(path) : le chemin du .oc.ui doit être connu à la compilation (littéral ou constante)".to_string()))?;
    let path = resolve(&raw, span);
    let spec = crate::core::ui_schema::check_file(&path).map_err(|errors| {
        let first = &errors[0];
        let mut at = Span::new(first.line, first.col);
        at.file = Some(first.file.display().to_string());
        let more = match errors.len() {
            1 => String::new(),
            n => format!(" (et {} autre(s) erreur(s) : `ocara {}`)", n - 1, path.display()),
        };
        (at, format!("{}{more}", first.message))
    })?;
    args[0] = Expr::Literal(Literal::String(spec.tree.to_string()), span.clone());
    Ok(())
}
