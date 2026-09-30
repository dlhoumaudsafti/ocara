/// Tests unitaires — arguments nommés à l'appel (E45 à E50) et leur
/// réécriture positionnelle — voir docs/roadmap.d/langage-named-arguments.md.
///
/// Repose sur un vrai pipeline lex → parse → symboles → typecheck →
/// réécriture (`core::named_args`), comme `method_call_on_void.rs`.
use crate::core::named_args::rewrite_named_args;
use crate::parsing::ast::{Expr, Literal, Program, Stmt};
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

/// Erreurs de la sema, puis programme réécrit si elle n'en a produit aucune.
fn check(src: &str) -> (Vec<SemaError>, Program) {
    let mut program = parse(src);
    let mut symbols = SymbolTable::new();
    for decl in &program.imports { symbols.register_import(decl); }
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.generics { symbols.register_generic(decl); }
    for decl in &program.functions { symbols.register_function(decl); }

    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    let errors = std::mem::take(&mut checker.errors);
    let rewrites = std::mem::take(&mut checker.named_arg_rewrites);
    if errors.is_empty() {
        rewrite_named_args(&mut program, &rewrites).expect("rewrite ok");
    }
    (errors, program)
}

/// Arguments de l'appel porté par la première instruction `var` de `main`.
fn main_first_call_args(program: &Program) -> &[Expr] {
    let main = program.functions.iter().find(|f| f.name == "main").expect("main");
    match &main.body.stmts[0] {
        Stmt::Var { value: Expr::Call { args, .. } | Expr::New { args, .. } | Expr::StaticCall { args, .. }, .. } => args,
        other => panic!("expected a call in a var, found {:?}", other),
    }
}

fn int_values(args: &[Expr]) -> Vec<i64> {
    args.iter()
        .map(|a| match a {
            Expr::Literal(Literal::Int(n), _) => *n,
            other => panic!("expected an int literal, found {:?}", other),
        })
        .collect()
}

const PICK: &str = r#"
    function pick(a:int, b:int = 20, c:int = 30):int { return a + b + c }
"#;

#[test]
fn parser_produces_named_arg_nodes() {
    let program = parse(&format!("{}\nfunction main():int {{ var x:int = pick(c: 3, a: 1)\n return 0 }}", PICK));
    let args = main_first_call_args(&program);
    assert!(matches!(&args[0], Expr::NamedArg { name, .. } if name == "c"));
    assert!(matches!(&args[1], Expr::NamedArg { name, .. } if name == "a"));
}

#[test]
fn named_call_is_rewritten_in_declaration_order_with_defaults() {
    let (errors, program) = check(&format!("{}\nfunction main():int {{ var x:int = pick(c: 3, a: 1)\n return x }}", PICK));
    assert!(errors.is_empty(), "{:?}", errors);
    assert_eq!(int_values(main_first_call_args(&program)), vec![1, 20, 3]);
}

#[test]
fn constructor_named_call_is_rewritten() {
    let src = r#"
        class P {
            public property a:int
            public property b:int
            init(a:int, b:int = 9) {
                self.a = a
                self.b = b
            }
        }
        function main():int { var p:P = use P(b: 2, a: 1)
            return 0 }
    "#;
    let (errors, program) = check(src);
    assert!(errors.is_empty(), "{:?}", errors);
    assert_eq!(int_values(main_first_call_args(&program)), vec![1, 2]);
}

#[test]
fn inherited_method_resolves_parent_parameter_names() {
    let src = r#"
        class Base { public method f(x:int, y:int):int { return x - y } }
        class Child extends Base { }
        function main():int {
            var c:Child = use Child()
            return c.f(y: 1, x: 5)
        }
    "#;
    assert!(check(src).0.is_empty());
}

#[test]
fn mixed_positional_and_named_is_rejected() {
    let (errors, _) = check(&format!("{}\nfunction main():int {{ var x:int = pick(1, c: 3)\n return x }}", PICK));
    assert!(errors.iter().any(|e| matches!(e, SemaError::NamedArgMixed { .. })), "{:?}", errors);
}

#[test]
fn unknown_name_lists_valid_names() {
    let (errors, _) = check(&format!("{}\nfunction main():int {{ var x:int = pick(z: 1)\n return x }}", PICK));
    let valid = errors.iter().find_map(|e| match e {
        SemaError::NamedArgUnknown { valid, .. } => Some(valid.clone()),
        _ => None,
    });
    assert_eq!(valid, Some(vec!["a".to_string(), "b".to_string(), "c".to_string()]));
}

#[test]
fn duplicate_name_is_rejected() {
    let (errors, _) = check(&format!("{}\nfunction main():int {{ var x:int = pick(a: 1, a: 2)\n return x }}", PICK));
    assert!(errors.iter().any(|e| matches!(e, SemaError::NamedArgDuplicate { .. })), "{:?}", errors);
}

#[test]
fn variadic_parameter_cannot_be_named() {
    let src = r#"
        function sum(label:string, nums:variadic<int>):string { return label }
        function main():int { var s:string = sum(label: "x", nums: 1)
            return 0 }
    "#;
    let (errors, _) = check(src);
    assert!(errors.iter().any(|e| matches!(e, SemaError::NamedArgVariadic { .. })), "{:?}", errors);
}

#[test]
fn missing_required_argument_is_rejected_without_arity_cascade() {
    let (errors, _) = check(&format!("{}\nfunction main():int {{ var x:int = pick(b: 1)\n return x }}", PICK));
    assert!(errors.iter().any(|e| matches!(e, SemaError::NamedArgMissing { name, .. } if name == "a")), "{:?}", errors);
    assert!(!errors.iter().any(|e| matches!(e, SemaError::WrongArgCount { .. })), "{:?}", errors);
}

#[test]
fn call_through_function_value_rejects_named_arguments() {
    let src = r#"
        function pair(a:int, b:int):int { return a - b }
        function main():int {
            var f:Function<int(int, int)> = pair
            return f(a: 1, b: 2)
        }
    "#;
    let (errors, _) = check(src);
    assert!(errors.iter().any(|e| matches!(e, SemaError::NamedArgUnresolved { .. })), "{:?}", errors);
}

#[test]
fn named_call_inside_generic_body_is_resolved_syntactically() {
    let src = r#"
        function tag(label:string, level:int = 1):string { return label }
        generic Holder<T> {
            public method label():string { return tag(level: 2, label: "h") }
        }
        function main():int { return 0 }
    "#;
    let (errors, program) = check(src);
    assert!(errors.is_empty(), "{:?}", errors);
    let crate::parsing::ast::ClassMember::Method { decl, .. } = &program.generics[0].members[0] else {
        panic!("expected a method");
    };
    let Stmt::Return { value: Some(Expr::Call { args, .. }), .. } = &decl.body.stmts[0] else {
        panic!("expected return of a call");
    };
    assert!(matches!(&args[0], Expr::Literal(Literal::String(s), _) if s == "h"));
    assert!(matches!(&args[1], Expr::Literal(Literal::Int(2), _)));
}

#[test]
fn constructor_arity_is_checked() {
    let src = r#"
        class P {
            public property a:int
            init(a:int, b:int = 1) { self.a = a }
        }
        function main():int { var p:P = use P()
            var q:P = use P(1, 2, 3)
            return 0 }
    "#;
    let (errors, _) = check(src);
    let arity_errors = errors.iter().filter(|e| matches!(e, SemaError::WrongArgCount { name, .. } if name == "P::init")).count();
    assert_eq!(arity_errors, 2, "{:?}", errors);
}
