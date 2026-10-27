/// Tests unitaires — accès par clé à une valeur `mixed` réécrit en
/// `Convert::mixedGet`/`Convert::mixedSet` (voir src/sema/mixed_access.rs).
use crate::core::named_args::rewrite_program;
use crate::parsing::ast::{Expr, Program, Stmt};
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn rewritten(main_body: &str) -> Program {
    let src = format!(r#"
        generic Box<T> {{
            public property v:T
            init(v:T) {{ self.v = v }}
            public method first(): mixed {{ return self.v["a"] }}
        }}
        function main(): int {{
            var x:mixed = null
            {}
            return 0
        }}
    "#, main_body);
    let tokens = Lexer::new(&src).tokenize().expect("lex ok");
    let mut program = Parser::new(tokens).parse_program().expect("parse ok");
    let mut symbols = SymbolTable::new();
    for decl in &program.imports { symbols.register_import(decl); }
    for decl in &program.generics { symbols.register_generic(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    assert!(checker.errors.is_empty(), "{:?}", checker.errors);
    let rewrites = std::mem::take(&mut checker.rewrites);
    rewrite_program(&mut program, &rewrites).expect("rewrite ok");
    program
}

/// Instruction `index` ajoutée au corps de `main` (après `var x`).
fn added_stmt(program: &Program, index: usize) -> &Stmt {
    let main = program.functions.iter().find(|f| f.name == "main").expect("main");
    &main.body.stmts[1 + index]
}

fn convert_call<'p>(expr: &'p Expr, method: &str) -> &'p [Expr] {
    match expr {
        Expr::StaticCall { class, method: m, args, .. } if class == "Convert" && m == method => args,
        other => panic!("expected Convert::{}, found {:?}", method, other),
    }
}

fn var_value(stmt: &Stmt) -> &Expr {
    match stmt {
        Stmt::Var { value, .. } => value,
        other => panic!("expected a var, found {:?}", other),
    }
}

#[test]
fn index_and_dot_reads_become_mixed_get() {
    let program = rewritten("var a:mixed = x[\"user\"]\n var b:mixed = x.user");
    assert_eq!(convert_call(var_value(added_stmt(&program, 0)), "mixedGet").len(), 2);
    assert_eq!(convert_call(var_value(added_stmt(&program, 1)), "mixedGet").len(), 2);
    assert!(program.imports.iter().any(|i| i.path == vec!["ocara".to_string(), "Convert".to_string()]));
}

#[test]
fn chained_index_reads_nest_mixed_get() {
    let program = rewritten("var a:mixed = x[\"tags\"][0]");
    let outer = convert_call(var_value(added_stmt(&program, 0)), "mixedGet");
    convert_call(&outer[0], "mixedGet");
}

#[test]
fn index_and_dot_writes_become_mixed_set() {
    let program = rewritten("x[\"n\"] = 1\n x.n = 2\n x[\"n\"] += 3");
    for i in 0..3 {
        match added_stmt(&program, i) {
            Stmt::Expr(call) => assert_eq!(convert_call(call, "mixedSet").len(), 3),
            other => panic!("expected Convert::mixedSet, found {:?}", other),
        }
    }
}

#[test]
fn catch_all_binding_keeps_its_field_access() {
    let program = rewritten("try { raise \"boom\" } on e { var m:mixed = e.message }");
    let Stmt::Try { handlers, .. } = added_stmt(&program, 0) else { panic!("expected a try") };
    assert!(matches!(var_value(&handlers[0].body.stmts[0]), Expr::Field { .. }));
}

#[test]
fn generic_body_is_not_rewritten() {
    let program = rewritten("");
    let body = &program.generics[0].members.iter().find_map(|m| match m {
        crate::parsing::ast::ClassMember::Method { decl, .. } if decl.name == "first" => Some(&decl.body),
        _ => None,
    }).expect("first").stmts[0];
    assert!(matches!(body, Stmt::Return { value: Some(Expr::Index { .. }), .. }));
}
