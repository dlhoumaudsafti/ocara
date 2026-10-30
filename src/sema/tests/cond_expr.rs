/// Tests unitaires — expression conditionnelle `then` et sa forme courte
/// `empty`/`zero`/`null`, voir src/sema/cond_expr.rs et `docs/EBNF.md` §11.3.
use crate::core::named_args::rewrite_program;
use crate::parsing::ast::{CondElse, Expr, Literal, Program, Stmt};
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn check(main_body: &str) -> (Vec<SemaError>, Program) {
    check_with("", main_body)
}

/// `declarations` : déclarations ajoutées avant `main`.
fn check_with(declarations: &str, main_body: &str) -> (Vec<SemaError>, Program) {
    let src = format!(r#"
        class User {{
            public property name:string = "x"
        }}
        {}
        function main(): int {{
            var ok:bool = true
            var s:string = "s"
            {}
            return 0
        }}
    "#, declarations, main_body);
    let tokens = Lexer::new(&src).tokenize().expect("lex ok");
    let mut program = Parser::new(tokens).parse_program().expect("parse ok");
    let mut symbols = SymbolTable::new();
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.generics { symbols.register_generic(decl); }
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

fn errors(main_body: &str) -> Vec<SemaError> {
    check(main_body).0
}

/// Branche `else` de la `index`-ième instruction `var` ajoutée à `main`.
fn else_of_added_var(program: &Program, index: usize) -> &Expr {
    let main = program.functions.iter().find(|f| f.name == "main").expect("main");
    match &main.body.stmts[2 + index] {
        Stmt::Var { value: Expr::Cond { else_expr: CondElse::Value(e), .. }, .. } => e,
        other => panic!("expected a rewritten conditional, found {:?}", other),
    }
}

#[test]
fn type_is_the_union_of_the_branches() {
    assert!(errors("var a:string|null = ok then s else null").is_empty());
    let errs = errors("var a:int = ok then 1 else 2.0");
    assert!(matches!(&errs[..], [SemaError::TypeMismatch { found, .. }] if found == "int|float"), "{:?}", errs);
}

#[test]
fn condition_follows_the_rules_of_if() {
    let errs = errors("var a:string = 3 then s else s");
    assert!(matches!(&errs[..], [SemaError::TypeMismatch { expected, .. }] if expected == "bool"), "{:?}", errs);
    assert!(errors("var m:mixed = 0\n var a:string = m then s else s").is_empty());
}

#[test]
fn condition_narrows_each_branch() {
    assert!(errors("var x:string|null = null\n var n:int = x is string then x.len() else 0").is_empty());
    assert!(errors("var u:User|null = null\n var n:string = u not equal null then u.name else \"\"").is_empty());
}

#[test]
fn defaults_become_literals_of_the_then_type() {
    let (errs, program) = check(concat!(
        "var a:string = ok then s empty\n",
        "var b:int = ok then 4 zero\n",
        "var c:float = ok then 4.5 zero\n",
        "var d:array<int> = ok then [1] empty\n",
        "var e:map<string, int> = ok then {\"k\": 1} empty\n",
        "var f:string|null = ok then s null",
    ));
    assert!(errs.is_empty(), "{:?}", errs);
    assert!(matches!(else_of_added_var(&program, 0), Expr::Literal(Literal::String(v), _) if v.is_empty()));
    assert!(matches!(else_of_added_var(&program, 1), Expr::Literal(Literal::Int(0), _)));
    assert!(matches!(else_of_added_var(&program, 2), Expr::Literal(Literal::Float(v), _) if *v == 0.0));
    assert!(matches!(else_of_added_var(&program, 3), Expr::Array { elements, .. } if elements.is_empty()));
    assert!(matches!(else_of_added_var(&program, 4), Expr::Map { entries, .. } if entries.is_empty()));
    assert!(matches!(else_of_added_var(&program, 5), Expr::Literal(Literal::Null, _)));
}

#[test]
fn null_default_makes_the_type_nullable() {
    let errs = errors("var a:string = ok then s null");
    assert!(matches!(&errs[..], [SemaError::TypeMismatch { found, .. }] if found == "string|null"), "{:?}", errs);
}

#[test]
fn default_without_such_value_is_e72() {
    for body in [
        "var a:string = ok then s zero",
        "var a:int = ok then 4 empty",
        "var u:User = use User()\n var a:User = ok then u empty",
        "var x:int|string = 3\n var a:int|string = ok then x zero",
        "var m:mixed = 1\n var a:mixed = ok then m zero",
    ] {
        let errs = errors(body);
        assert!(matches!(&errs[..], [SemaError::CondDefaultType { .. }]), "{} : {:?}", body, errs);
    }
}

#[test]
fn default_on_a_type_parameter_is_e73_but_null_is_allowed() {
    let generic = r#"
        generic Box<T> {
            public property v:T
            init(v:T) { self.v = v }
            public method orZero(ok:bool): T { return ok then self.v zero }
            public method orNull(ok:bool): T|null { return ok then self.v null }
        }"#;
    let errs = check_with(generic, "var b:Box<int> = use Box<int>(1)").0;
    assert!(matches!(&errs[..], [SemaError::CondDefaultOnTypeParam { keyword, .. }] if keyword == "zero"), "{:?}", errs);
}
