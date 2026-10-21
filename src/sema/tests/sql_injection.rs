/// Tests unitaires — W06, requête SQL construite par interpolation ou
/// concaténation (docs/diagnostics.md, W06).
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaWarning;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn sql_warnings(body: &str) -> usize {
    let src = format!("import ocara.SQLite\nfunction main(): int {{\n    var id:int = 3\n    var db:SQLite = SQLite::open(\":memory:\")\n{}\n    db.close()\n    return id\n}}\n", body);
    let tokens = Lexer::new(&src).tokenize().expect("lex ok");
    let program = Parser::new(tokens).parse_program().expect("parse ok");
    let mut symbols = SymbolTable::new();
    for decl in &program.imports { symbols.register_import(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.warnings.iter().filter(|w| matches!(w, SemaWarning::SqlInterpolation { .. })).count()
}

#[test]
fn template_interpolation_warns() {
    assert_eq!(sql_warnings("    db.execute(`DELETE FROM t WHERE id = ${id}`)"), 1);
    assert_eq!(sql_warnings("    var rows:array<map<string, mixed>> = db.query(`SELECT * FROM t WHERE id = ${id}`)"), 1);
}

#[test]
fn concatenation_warns() {
    assert_eq!(sql_warnings("    db.prepare(\"SELECT * FROM t WHERE name = \" + id.toStr())"), 1);
}

#[test]
fn placeholders_and_constant_queries_do_not_warn() {
    assert_eq!(sql_warnings("    db.execute(\"DELETE FROM t WHERE id = :id\", {\"id\": id})"), 0);
    assert_eq!(sql_warnings("    db.execute(`CREATE TABLE t (id INTEGER)`)"), 0);
}
