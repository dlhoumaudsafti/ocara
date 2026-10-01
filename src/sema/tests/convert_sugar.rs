/// Tests unitaires — méthodes d'instance de conversion (`s.toInt()` →
/// `Convert::strToInt(s)`) — voir docs/roadmap.d/stdlib-convert-instance-methods.md.
use crate::core::named_args::rewrite_program;
use crate::parsing::ast::{Expr, Program, Stmt, Type};
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::convert_sugar::convert_method_for;
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn check(src: &str) -> (Vec<SemaError>, Program) {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    let mut program = Parser::new(tokens).parse_program().expect("parse ok");
    let mut symbols = SymbolTable::new();
    for decl in &program.imports { symbols.register_import(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    let errors = std::mem::take(&mut checker.errors);
    let rewrites = std::mem::take(&mut checker.rewrites);
    if errors.is_empty() {
        rewrite_program(&mut program, &rewrites).expect("rewrite ok");
    }
    (errors, program)
}

fn main_var_value(program: &Program, index: usize) -> &Expr {
    let main = program.functions.iter().find(|f| f.name == "main").expect("main");
    match &main.body.stmts[index] {
        Stmt::Var { value, .. } => value,
        other => panic!("expected a var, found {:?}", other),
    }
}

#[test]
fn mapping_covers_the_seventeen_conversions_only() {
    let receivers = [Type::String, Type::Int, Type::Float, Type::Bool, Type::Array(Box::new(Type::Int)), Type::Map(Box::new(Type::String), Box::new(Type::String))];
    let names = ["toInt", "toFloat", "toBool", "toStr", "toArray", "toMap", "keysToArray"];
    let count = receivers.iter().flat_map(|r| names.iter().filter_map(move |n| convert_method_for(r, n))).count();
    assert_eq!(count, 17);
    assert_eq!(convert_method_for(&Type::Int, "toInt"), None);
}

#[test]
fn instance_call_is_rewritten_to_convert_static_call_and_import_added() {
    let (errors, program) = check("function main(): int { var s:string = \"1\"\n var n:int = s.toInt()\n return n }");
    assert!(errors.is_empty(), "{:?}", errors);
    match main_var_value(&program, 1) {
        Expr::StaticCall { class, method, args, .. } => {
            assert_eq!((class.as_str(), method.as_str(), args.len()), ("Convert", "strToInt", 1));
        }
        other => panic!("expected Convert::strToInt, found {:?}", other),
    }
    assert!(program.imports.iter().any(|i| i.path == vec!["ocara".to_string(), "Convert".to_string()]));
}

#[test]
fn chained_conversions_are_all_rewritten() {
    let (errors, program) = check("function main(): int { var s:string = \"1\"\n var t:string = s.toInt().toStr()\n return 0 }");
    assert!(errors.is_empty(), "{:?}", errors);
    let Expr::StaticCall { method, args, .. } = main_var_value(&program, 1) else { panic!("expected a static call") };
    assert_eq!(method, "intToStr");
    assert!(matches!(&args[0], Expr::StaticCall { method, .. } if method == "strToInt"));
}

#[test]
fn return_type_is_the_convert_return_type() {
    let (errors, _) = check("function main(): int { var f:float = 2.5\n var s:string = f.toInt()\n return 0 }");
    assert!(errors.iter().any(|e| matches!(e, SemaError::TypeMismatch { .. })), "{:?}", errors);
}

#[test]
fn wrong_arity_is_rejected() {
    let (errors, _) = check("function main(): int { var s:string = \"1\"\n var n:int = s.toInt(2)\n return n }");
    assert!(errors.iter().any(|e| matches!(e, SemaError::WrongArgCount { .. })), "{:?}", errors);
}
