/// Tests unitaires — solidité du typage (docs/roadmap.d/sema-solidite-typage.md) :
/// `null` réservé à `T|null`, union « trouvée » (toutes les variantes),
/// variance des conteneurs et de `Function`, narrowing, `match`, `-`,
/// indexation, chemins sans `return`, redéfinitions, doublons, handler mort.
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

/// (erreurs, avertissements, valeurs `mixed` vérifiées à l'exécution)
fn check(src: &str) -> (Vec<String>, Vec<String>, usize) {
    let program = parse(src);
    let mut symbols = SymbolTable::new();
    for decl in &program.imports { symbols.register_import(decl); }
    for decl in &program.enums { symbols.register_enum(decl); }
    for decl in &program.generics { symbols.register_generic(decl); }
    for decl in &program.interfaces { symbols.register_interface(decl); }
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.functions { symbols.register_function(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    (
        checker.errors.iter().map(|e| e.message()).collect(),
        checker.warnings.iter().map(|w| w.message()).collect(),
        checker.rewrites.mixed_checks.len(),
    )
}

fn errors(src: &str) -> Vec<String> {
    check(src).0
}

const ANIMALS: &str = "class Animal { public property n:int = 1 }\nclass Dog extends Animal { public property t:int = 2 }\n";

#[test]
fn null_needs_a_nullable_type() {
    let ko = errors("class A { public property v:int = 1 }\nfunction mk(): A { return null }\nfunction main(): int {\n var a:A = null\n var s:string = null\n return 0\n}\n");
    assert_eq!(ko.iter().filter(|e| e.contains("found 'null'")).count(), 3, "{:?}", ko);
    let ok = errors("class A { public property v:int = 1 }\nfunction mk(): A|null { return null }\nfunction main(): int {\n var a:A|null = null\n var s:string|null = null\n if s equal null { return 1 }\n return 0\n}\n");
    assert!(ok.is_empty(), "{:?}", ok);
}

#[test]
fn every_union_variant_must_fit() {
    let ko = errors("function main(): int {\n var x:int|string = \"a\"\n var y:int = x\n return y\n}\n");
    assert!(ko.iter().any(|e| e.contains("expected type 'int', found 'int|string'")), "{:?}", ko);
}

#[test]
fn narrowing_by_is_null_and_is_type() {
    let ok = errors("function len(s:string|null): int {\n if s is null {\n return 0\n }\n var t:string = s\n return 1\n}\nfunction pick(x:int|string): int {\n if x is int {\n var n:int = x\n return n\n }\n var s:string = x\n return 0\n}\nfunction main(): int {\n return len(null) + pick(2)\n}\n");
    assert!(ok.is_empty(), "{:?}", ok);
    let reassigned = errors("function f(s:string|null): string {\n if s is null { return \"\" }\n s = null\n return s\n}\nfunction main(): int { return 0 }\n");
    assert!(!reassigned.is_empty(), "une affectation annule le raffinement");
}

#[test]
fn containers_are_invariant() {
    let ko = errors(&format!("{}function main(): int {{\n var dogs:array<Dog> = [use Dog()]\n var animals:array<Animal> = dogs\n var m:map<string, Dog> = {{\"a\": use Dog()}}\n var ma:map<string, Animal> = m\n return 0\n}}\n", ANIMALS));
    assert_eq!(ko.iter().filter(|e| e.contains("expected type")).count(), 2, "{:?}", ko);
    let literal = errors(&format!("{}function main(): int {{\n var animals:array<Animal> = [use Dog(), use Animal()]\n var nums:array<int|float> = [1, 2.5]\n return 0\n}}\n", ANIMALS));
    assert!(literal.is_empty(), "{:?}", literal);
}

#[test]
fn function_parameters_are_contravariant() {
    let ko = errors(&format!("{}function main(): int {{\n var f:Function<int(Animal)> = nameless(d:Dog): int {{ return d.t }}\n return 0\n}}\n", ANIMALS));
    assert!(ko.iter().any(|e| e.contains("Function<int(Animal)>")), "{:?}", ko);
    let ok = errors(&format!("{}function main(): int {{\n var f:Function<int(Dog)> = nameless(a:Animal): int {{ return a.n }}\n return 0\n}}\n", ANIMALS));
    assert!(ok.is_empty(), "{:?}", ok);
}

#[test]
fn match_type_is_the_union_of_its_arms() {
    let ko = errors("class U { public property n:int = 1 }\nfunction main(): int {\n var n:int = 5\n var u:U = match n { 5 => \"x\"  default => use U() }\n var s:string = match n { 1 => \"un\" }\n return 0\n}\n");
    assert!(ko.iter().any(|e| e.contains("found 'string|U'")), "{:?}", ko);
    assert!(ko.iter().any(|e| e.contains("found 'string|null'")), "{:?}", ko);
}

#[test]
fn negation_and_string_indexing_are_rejected() {
    let ko = errors("function main(): int {\n var b:bool = true\n var s:string = \"abc\"\n var x:bool = -b\n var c:string = s[1]\n return 0\n}\n");
    assert!(ko.iter().any(|e| e.contains("operator '-' cannot be applied to 'bool'")), "{:?}", ko);
    assert!(ko.iter().any(|e| e.contains("cannot index a value of type 'string'")), "{:?}", ko);
}

#[test]
fn every_path_must_return() {
    let ko = errors("function g(x:int): string {\n if x greater 0 { return \"pos\" }\n}\nfunction main(): int {\n var f:Function<int()> = nameless(): int { if true { return 1 } }\n return 0\n}\n");
    assert_eq!(ko.iter().filter(|e| e.contains("on every path")).count(), 2, "{:?}", ko);
    let ok = errors("function g(x:int): string {\n if x greater 0 { return \"pos\" } else { raise \"neg\" }\n}\nfunction h(): int {\n while true { return 1 }\n}\nfunction main(): int { return 0 }\n");
    assert!(ok.is_empty(), "{:?}", ok);
}

#[test]
fn redefinition_keeps_a_compatible_signature() {
    let ko = errors("class T { public property x:int = 1 }\nclass A { public method v(): int { return 1 }\n public method w(n:int): int { return n } }\nclass B extends A { public method v(): T { return use T() }\n public method w(n:string): int { return 0 } }\nfunction main(): int { return 0 }\n");
    assert!(ko.iter().any(|e| e.contains("returns 'T' instead of 'int'")), "{:?}", ko);
    assert!(ko.iter().any(|e| e.contains("parameter 'n' is 'string' instead of 'int'")), "{:?}", ko);
    let ok = errors(&format!("{}class Shelter {{ public method get(): Animal {{ return use Animal() }} }}\nclass Kennel extends Shelter {{ public method get(): Dog {{ return use Dog() }} }}\nfunction main(): int {{ return 0 }}\n", ANIMALS));
    assert!(ok.is_empty(), "retour covariant accepté : {:?}", ok);
}

#[test]
fn duplicate_declarations_are_reported() {
    let ko = errors("function foo(): int { return 1 }\nfunction foo(): int { return 2 }\nclass A { public method m(): int { return 1 }\n public method m(): int { return 2 } }\nfunction main(): int { return 0 }\n");
    assert!(ko.iter().any(|e| e == "duplicate symbol 'foo'"), "{:?}", ko);
    assert!(ko.iter().any(|e| e == "duplicate symbol 'A::m'"), "{:?}", ko);
}

#[test]
fn handler_after_its_parent_is_unreachable() {
    let (_, warnings, _) = check("class P { public property message:string = \"p\" }\nclass E extends P { }\nfunction main(): int {\n try { raise use E() } on e is P { return 1 } on e is E { return 2 }\n return 0\n}\n");
    assert!(warnings.iter().any(|w| w.contains("handler 'on … is E' is never reached")), "{:?}", warnings);
}

#[test]
fn generic_field_assignment_is_checked() {
    let ko = errors("generic Box<T> {\n public property v:T\n init(v:T) { self.v = v }\n}\nfunction main(): int {\n var b:Box<int> = use Box<int>(3)\n b.v = \"zz\"\n return 0\n}\n");
    assert!(ko.iter().any(|e| e.contains("expected type 'int', found 'string'")), "{:?}", ko);
}

#[test]
fn mixed_into_a_concrete_type_is_checked_at_runtime() {
    let (errs, _, checks) = check("class U { public property n:int = 1 }\nfunction main(): int {\n var arr:array<mixed> = [1, \"s\"]\n var u:U = arr[1]\n var i:int = arr[0]\n return i\n}\n");
    assert!(errs.is_empty(), "{:?}", errs);
    assert_eq!(checks, 2);
}
