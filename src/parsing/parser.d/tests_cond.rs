//! Tests de l'expression conditionnelle `then` (voir cond.rs).

use crate::parsing::ast::*;
use crate::parsing::lexer::Lexer;
use crate::parsing::parser::Parser;

fn parse_expr(src: &str) -> Expr {
    let tokens = Lexer::new(src).tokenize().expect("lex error");
    Parser::new(tokens).parse_expr().expect("parse error")
}

fn parse(src: &str) -> Result<Program, String> {
    let tokens = Lexer::new(src).tokenize().expect("lex error");
    Parser::new(tokens).parse_program().map_err(|e| e.message)
}

/// Branches d'un `Expr::Cond`.
fn cond_parts(expr: &Expr) -> (&Expr, &Expr, &CondElse) {
    match expr {
        Expr::Cond { cond, then_expr, else_expr, .. } => (cond, then_expr, else_expr),
        other => panic!("expression conditionnelle attendue : {:?}", other),
    }
}

fn is_ident(expr: &Expr, name: &str) -> bool {
    matches!(expr, Expr::Ident(n, _) if n == name)
}

/// Valeur de la première instruction `var` de `main`.
fn first_var(program: &Program) -> &Expr {
    match &program.functions[0].body.stmts[0] {
        Stmt::Var { value, .. } => value,
        other => panic!("var attendu : {:?}", other),
    }
}

#[test]
fn else_branch_takes_the_rest_of_the_expression() {
    let expr = parse_expr("a then b else c + d");
    let (_, _, else_expr) = cond_parts(&expr);
    assert!(matches!(else_expr.value(), Some(Expr::Binary { op: BinOp::Add, .. })));
}

#[test]
fn is_right_associative() {
    let expr = parse_expr("x then a else y then b else c");
    let (cond, _, else_expr) = cond_parts(&expr);
    assert!(is_ident(cond, "x"));
    let (inner, _, _) = cond_parts(else_expr.value().expect("else"));
    assert!(is_ident(inner, "y"));
}

#[test]
fn has_the_lowest_precedence() {
    let expr = parse_expr("a or b and c equal 1 then 1 else 2");
    let (cond, _, _) = cond_parts(&expr);
    assert!(matches!(cond, Expr::Binary { op: BinOp::Or, .. }));
}

#[test]
fn parentheses_isolate_the_result() {
    let Expr::Binary { op: BinOp::Add, left, .. } = parse_expr("(a then b else c) + d") else { panic!("addition attendue") };
    cond_parts(&left);
}

#[test]
fn spans_several_lines_and_nests_in_the_then_branch() {
    let expr = parse_expr("(a\n  then (b\n    then 1\n    else 2\n  )\n  else 3\n) + 1");
    let Expr::Binary { left, .. } = expr else { panic!("addition attendue") };
    let (_, then_expr, _) = cond_parts(&left);
    cond_parts(then_expr);
}

#[test]
fn then_without_else_or_default_is_an_error() {
    let err = parse("function main(): void {\n    var s:string = a then \"x\"\n}\n").expect_err("accepté");
    assert!(err.contains("'then' expects 'else <value>'"), "{}", err);
}

#[test]
fn short_forms_are_contextual_defaults() {
    for (src, kind) in [("c then v empty", CondDefault::Empty), ("c then v zero", CondDefault::Zero), ("c then v null", CondDefault::Null)] {
        let expr = parse_expr(src);
        let (_, _, else_expr) = cond_parts(&expr);
        assert!(matches!(else_expr, CondElse::Default(k, _) if *k == kind), "{}", src);
    }
}

#[test]
fn default_may_start_the_next_line() {
    let expr = parse_expr("c\n  then v\n  empty");
    let (_, _, else_expr) = cond_parts(&expr);
    assert!(matches!(else_expr, CondElse::Default(CondDefault::Empty, _)));
}

#[test]
fn keyword_and_identifier_zero_in_the_same_expression() {
    let expr = parse_expr("zero equal 0 then zero zero");
    let (cond, then_expr, else_expr) = cond_parts(&expr);
    assert!(matches!(cond, Expr::Binary { op: BinOp::Equal, left, .. } if is_ident(left, "zero")));
    assert!(is_ident(then_expr, "zero"));
    assert!(matches!(else_expr, CondElse::Default(CondDefault::Zero, _)));
}

#[test]
fn else_empty_reads_a_variable() {
    let expr = parse_expr("c then v else empty");
    let (_, _, else_expr) = cond_parts(&expr);
    assert!(is_ident(else_expr.value().expect("else"), "empty"));
}

#[test]
fn empty_and_zero_stay_identifiers_elsewhere() {
    let program = parse("function main(): void {\n    var empty:array<int> = []\n    var zero:int = String::empty(s) + m.zero()\n}\n").expect("parse");
    assert_eq!(program.functions[0].body.stmts.len(), 2);
    assert!(program.cond_keywords.is_empty());
}

#[test]
fn records_the_position_of_each_default_keyword() {
    let program = parse("function main(): void {\n    var a:int = zero equal 0 then zero zero\n}\n").expect("parse");
    let positions: Vec<(usize, usize)> = program.cond_keywords.iter().map(|s| (s.line, s.col)).collect();
    assert_eq!(positions, vec![(2, 40)]);
    cond_parts(first_var(&program));
}

#[test]
fn works_in_if_conditions_and_if_bodies() {
    let program = parse("function main(): void {\n    if a then b else c {\n        x = c then 1 else 2\n    } else {\n        x = 3\n    }\n}\n").expect("parse");
    let Stmt::If { condition, else_block, .. } = &program.functions[0].body.stmts[0] else { panic!("if attendu") };
    cond_parts(condition);
    assert!(else_block.is_some());
}

#[test]
fn null_default_in_a_match_arm_before_a_null_pattern() {
    let src = "function main(): void {\n    var r:string|null = match k {\n        1 => c then \"a\" null\n        null => \"n\"\n    }\n}\n";
    let program = parse(src).expect("parse");
    let Expr::Match { arms, .. } = first_var(&program) else { panic!("match attendu") };
    assert_eq!(arms.len(), 2);
    cond_parts(&arms[0].body);
}
