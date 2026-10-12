//! Lieu de déclaration de chaque fonction du programme, sous son nom IR
//! (`fonction`, `Classe_méthode`, `Classe_init`, `main` des blocs runtime) :
//! localisation des fuites rapportées par `--leak-check` (ir/leak_check.rs).

use std::collections::HashMap;
use std::path::Path;

use crate::parsing::ast::{ClassMember, Program};
use crate::parsing::token::Span;

pub fn sites(program: &Program, input: &Path) -> HashMap<String, (String, usize)> {
    let file_of = |span: &Span| span.file.clone().unwrap_or_else(|| input.to_string_lossy().into_owned());
    let mut out: HashMap<String, (String, usize)> = HashMap::new();
    for f in &program.functions {
        out.insert(f.name.clone(), (file_of(&f.span), f.span.line));
    }
    let owners = program.classes.iter().map(|c| (&c.name, &c.members))
        .chain(program.modules.iter().map(|m| (&m.name, &m.members)));
    for (owner, members) in owners {
        for member in members {
            match member {
                ClassMember::Method { decl, .. } => {
                    out.insert(format!("{}_{}", owner, decl.name), (file_of(&decl.span), decl.span.line));
                }
                ClassMember::Constructor { span, .. } => {
                    out.insert(format!("{}_init", owner), (file_of(span), span.line));
                }
                _ => {}
            }
        }
    }
    if let Some(block) = program.runtime_blocks.iter().find(|b| b.kind.as_str() == "main") {
        out.entry("main".into()).or_insert((file_of(&block.span), block.span.line));
    }
    out
}
