/// Tests unitaires — `core::monomorph` : instanciations découvertes dans le
/// corps d'un générique instancié (point fixe) — voir
/// docs/roadmap.d/langage-dispatch-heritage-generiques.md.
use crate::core::monomorph::{generic_instantiations, monomorphize};
use crate::parsing::ast::{Program, Type};
use crate::parsing::{lexer::Lexer, parser::Parser};

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

const SWAP: &str = "generic Pair<A, B> {\n\
    public property first:A\n\
    public property second:B\n\
    init(a:A, b:B) { self.first = a\n self.second = b }\n\
    public method swap(): Pair<B, A> { return use Pair<B, A>(self.second, self.first) }\n\
}\n\
function main(): int {\n\
    var p:Pair<int, string> = use Pair<int, string>(1, \"x\")\n\
    return 0\n\
}\n";

#[test]
fn instantiation_only_in_a_generic_body_is_collected() {
    let found = generic_instantiations(&parse(SWAP));
    assert!(found.contains(&("Pair".into(), vec![Type::Int, Type::String])));
    assert!(found.contains(&("Pair".into(), vec![Type::String, Type::Int])));
}

#[test]
fn instantiation_only_in_a_generic_body_is_specialized() {
    let mut program = parse(SWAP);
    monomorphize(&mut program);
    for name in ["Pair_int_string", "Pair_string_int"] {
        assert!(program.classes.iter().any(|c| c.name == name), "missing specialized class {}", name);
    }
}

#[test]
fn polymorphic_recursion_is_bounded() {
    let mut program = parse("generic Nest<T> {\n\
        public method deeper(): int { var n:Nest<array<T>> = use Nest<array<T>>()\n return 0 }\n\
    }\n\
    function main(): int {\n\
        var n:Nest<int> = use Nest<int>()\n\
        return 0\n\
    }\n");
    monomorphize(&mut program);
    assert!(program.classes.len() < 40, "polymorphic recursion must stop: {} classes", program.classes.len());
}
