/// Tests unitaires — affectations composées (`parser.d/compound_assign.rs`) :
/// réécriture en `x = x op e`, cible avec appel refusée (E58), `-=` sur
/// string noté pour `String::replace`, arithmétique sur un opérande non
/// numérique refusée (E59).
use crate::parsing::ast::{BinOp, Expr, Program, Stmt};
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Result<Program, String> {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().map_err(|e| format!("{:?}", e))
}

fn in_main(body: &str) -> String {
    format!("function next(): int {{ return 0 }}\nfunction main(): int {{\n    {}\n    return 0\n}}\n", body)
}

fn check(src: &str) -> TypeChecker<'static> {
    let program = Box::leak(Box::new(parse(src).expect("parse ok")));
    let symbols = Box::leak(Box::new(SymbolTable::new()));
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(symbols);
    checker.check_program(program);
    checker
}

#[test]
fn compound_assignment_is_rewritten_as_assignment() {
    let program = parse(&in_main("var n:int = 1\n    n *= 3")).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    let Stmt::Assign { target: Expr::Ident(name, _), value: Expr::Binary { op, left, .. }, .. } = &main.body.stmts[1] else {
        panic!("{:?}", main.body.stmts[1]);
    };
    assert_eq!((name.as_str(), op), ("n", &BinOp::Mul));
    assert!(matches!(left.as_ref(), Expr::Ident(l, _) if l == "n"));
}

#[test]
fn target_with_a_call_is_rejected() {
    let err = parse(&in_main("var a:array<int> = [1]\n    a[next()] += 1")).unwrap_err();
    assert!(err.contains("cannot contain a call"), "{}", err);
}

#[test]
fn string_removal_is_recorded() {
    let checker = check(&in_main("var s:string = \"a-b\"\n    s -= \"-\""));
    assert!(checker.errors.is_empty(), "{:?}", checker.errors);
    assert_eq!(checker.rewrites.string_removals.len(), 1);
}

#[test]
fn numeric_removal_stays_a_subtraction() {
    let checker = check(&in_main("var n:int = 3\n    n -= 1"));
    assert!(checker.errors.is_empty(), "{:?}", checker.errors);
    assert!(checker.rewrites.string_removals.is_empty());
}

#[test]
fn arithmetic_on_non_numeric_operands_is_rejected() {
    for body in ["var s:string = \"ab\" - \"b\"", "var s:string = \"x\"\n    s *= 2", "var b:bool = true\n    b += true", "var a:array<int> = [1]\n    a -= [1]"] {
        let checker = check(&in_main(body));
        assert!(checker.errors.iter().any(|e| matches!(e, SemaError::ArithmeticOnNonNumeric { .. })), "{}: {:?}", body, checker.errors);
    }
}
