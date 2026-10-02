// ─────────────────────────────────────────────────────────────────────────────
// Déclarations nommées d'un fichier (R07/R08/R09/R12/R13) et le style attendu
// pour chacune — partagé par l'analyse et par `--fix` (renommage).
// ─────────────────────────────────────────────────────────────────────────────

use crate::config::Config;
use crate::naming::Style;
use crate::scope::{is_callable_decl, BodyTracker};
use crate::text::{leading_name, strip_visibility, LineState};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeclKind {
    /// class/struct/interface/module/generic — renommer peut renommer le fichier.
    Type,
    /// Fonction, variable, constante globale ou locale : référencée sans `.`.
    Bare,
    /// Méthode, propriété, constante de classe : référencée aussi après `.`/`::`.
    Member,
}

#[derive(Debug)]
pub struct Decl {
    /// Numéro de ligne (1-based).
    pub line:  usize,
    pub name:  String,
    pub label: &'static str,
    pub style: Style,
    pub kind:  DeclKind,
}

impl Decl {
    pub fn issue(&self) -> Option<String> {
        (!self.style.matches(&self.name)).then(|| format!(
            "{} '{}' devrait être en {} → {}", self.label, self.name, self.style.label(), self.style.convert(&self.name)
        ))
    }
}

const TYPE_KINDS: &[(&str, &str)] = &[
    ("class ",     "classe"),
    ("interface ", "interface"),
    ("module ",    "module"),
    ("generic ",   "generic"),
    ("struct ",    "struct"),
];

/// Déclarations soumises à une règle de nommage ACTIVE.
pub fn declarations(file_name: &str, lines: &[&str], states: &[LineState], cfg: &Config) -> Vec<Decl> {
    let mut bodies = BodyTracker::for_file(file_name);
    let mut out = Vec::new();
    for (i, (line, state)) in lines.iter().zip(states).enumerate() {
        let in_body = bodies.in_body();
        bodies.advance(line);
        if state.starts_in_bt { continue; }
        let mut push = |name: &str, label, style, kind| {
            if !name.is_empty() {
                out.push(Decl { line: i + 1, name: name.to_string(), label, style, kind });
            }
        };
        let t = line.trim();
        let after_vis = strip_visibility(t);
        let has_vis = after_vis.len() != t.len();

        if cfg.naming_class {
            if let Some((kw, label)) = TYPE_KINDS.iter().find(|(kw, _)| t.starts_with(kw)) {
                push(leading_name(&t[kw.len()..]), label, cfg.class_style, DeclKind::Type);
                continue;
            }
        }
        if is_callable_decl(t) {
            let rest = after_vis.strip_prefix("static ").unwrap_or(after_vis);
            let rest = rest.strip_prefix("async ").unwrap_or(rest);
            if cfg.naming_function {
                if let Some(r) = rest.strip_prefix("function ") {
                    push(leading_name(r), "fonction", cfg.function_style, DeclKind::Bare);
                } else if let Some(r) = rest.strip_prefix("method ") {
                    push(leading_name(r), "méthode", cfg.function_style, DeclKind::Member);
                }
            }
            continue;
        }
        if let Some(r) = after_vis.strip_prefix("const ") {
            if in_body {
                if cfg.naming_const_embed {
                    push(leading_name(r), "constante locale", cfg.embed_style(), DeclKind::Bare);
                }
            } else if cfg.naming_const {
                let kind = if has_vis { DeclKind::Member } else { DeclKind::Bare };
                push(leading_name(r), "constante", cfg.const_style, kind);
            }
            continue;
        }
        if cfg.naming_variable {
            if let Some(r) = ["var ", "scoped ", "consumed "].iter().find_map(|kw| t.strip_prefix(kw)) {
                push(leading_name(r), "variable", cfg.var_style, DeclKind::Bare);
            } else if let Some(r) = after_vis.strip_prefix("property ") {
                push(leading_name(r), "propriété", cfg.var_style, DeclKind::Member);
            }
        }
    }
    out
}
