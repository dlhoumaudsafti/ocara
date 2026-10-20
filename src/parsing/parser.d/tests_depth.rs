/// Tests du parser — profondeur maximale d'une expression (voir
/// `MAX_EXPR_DEPTH`, docs/roadmap.d/compilateur-limites-echelle.md).
use crate::parsing::lexer::Lexer;
use crate::parsing::parser::Parser;

/// L'arbre (et sa libération récursive) dépasse la pile d'un thread de
/// test : même pile que le thread de compilation.
fn parse_on_big_stack(src: String) -> Result<(), String> {
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let tokens = Lexer::new(&src).tokenize().expect("lex error");
            Parser::new(tokens).parse_program().map(|_| ()).map_err(|e| e.message)
        })
        .expect("thread")
        .join()
        .expect("le parser ne doit pas déborder")
}

fn program(expr: &str) -> String {
    format!("function main(): int {{\n    var a:int = 1\n    var r:int = {}\n    return r\n}}\n", expr)
}

#[test]
fn long_operator_chain_within_limit_is_accepted() {
    assert!(parse_on_big_stack(program(&vec!["a"; 20_000].join(" + "))).is_ok());
}

#[test]
fn deep_nesting_within_limit_is_accepted() {
    let depth = 10_000;
    assert!(parse_on_big_stack(program(&format!("{}a{}", "(".repeat(depth), ")".repeat(depth)))).is_ok());
}

#[test]
fn too_long_operator_chain_is_a_diagnostic() {
    let err = parse_on_big_stack(program(&vec!["a"; 60_000].join(" + "))).expect_err("chaîne trop longue acceptée");
    assert!(err.contains("expression trop profonde"), "{}", err);
}

#[test]
fn nesting_plus_chain_beyond_limit_is_a_diagnostic() {
    let depth = 5_000;
    let chain = vec!["a"; 46_000].join(" + ");
    let err = parse_on_big_stack(program(&format!("{}{}{}", "(".repeat(depth), chain, ")".repeat(depth)))).expect_err("expression trop profonde acceptée");
    assert!(err.contains("expression trop profonde"), "{}", err);
}
