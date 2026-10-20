//! Tests — conflits de renommage (`rename_check`) : un nom déjà déclaré dans
//! la fonction, la classe ou le fichier, ou une classe builtin, est refusé.

use super::rename_check::conflict;
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};

const SOURCE: &str = "class Counter {\n    public property total:int = 0\n    public method add(step:int): int {\n        var count:int = step\n        for item in [1, 2] {\n            count = count + item\n        }\n        return count\n    }\n}\nfunction helper(): int {\n    var other:int = 1\n    return other\n}\n";

fn program() -> Program {
    Parser::new(Lexer::new(SOURCE).tokenize().expect("lex ok")).parse_program().expect("parse ok")
}

#[test]
fn local_renamed_onto_an_existing_local_is_refused() {
    assert!(conflict(&program(), 4, "item").is_some());
    assert!(conflict(&program(), 4, "step").is_some());
}

#[test]
fn local_renamed_onto_a_member_or_top_level_name_is_refused() {
    assert!(conflict(&program(), 4, "total").is_some());
    assert!(conflict(&program(), 4, "helper").is_some());
}

#[test]
fn builtin_class_name_is_refused() {
    assert!(conflict(&program(), 4, "IO").unwrap().contains("builtin"));
}

#[test]
fn free_name_is_accepted() {
    assert!(conflict(&program(), 4, "amount").is_none());
    // `other` n'est déclaré que dans `helper` : libre dans `add`.
    assert!(conflict(&program(), 4, "other").is_none());
}
