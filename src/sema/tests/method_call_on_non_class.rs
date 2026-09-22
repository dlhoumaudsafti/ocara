/// Tests unitaires — E37, appel de méthode sur un récepteur `int`/`float`/
/// `bool`/`null`/`message<T>`/`Function<...>` — voir
/// docs/roadmap.d/langage-appel-methode-sur-primitif-accepte.md.
///
/// Même mécanisme que E36 (`method_call_on_void.rs`), généralisé aux types
/// que ce correctif avait délibérément laissés de côté : `type_class_name`
/// retombait sur `None` pour eux aussi, silencieusement permissif (retourne
/// `Type::Mixed` sans vérifier `field`/`args`) — reproduit avec
/// `f.getCount().upper()` (`getCount(): int`), qui compilait sans erreur et
/// affichait `null` à l'exécution au lieu d'être rejeté.
///
/// Repose sur un vrai pipeline lex → parse → symboles → typecheck (comme
/// `method_call_on_void.rs`).
use crate::parsing::ast::Program;
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::error::SemaError;
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().expect("lex ok");
    Parser::new(tokens).parse_program().expect("parse ok")
}

fn check_errors(src: &str) -> Vec<SemaError> {
    let program = parse(src);
    let mut symbols = SymbolTable::new();
    for decl in &program.imports { symbols.register_import(decl); }
    for decl in &program.consts { symbols.register_const(decl); }
    for decl in &program.interfaces { symbols.register_interface(decl); }
    for decl in &program.modules { symbols.register_module(decl); }
    for decl in &program.enums { symbols.register_enum(decl); }
    for decl in &program.classes { symbols.register_class(decl); }
    for decl in &program.generics { symbols.register_generic(decl); }
    for decl in &program.functions { symbols.register_function(decl); }

    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    checker.errors
}

fn method_call_on_non_class(errors: &[SemaError]) -> Option<(&str, &str)> {
    errors.iter().find_map(|e| match e {
        SemaError::MethodCallOnNonClass { type_name, method, .. } => Some((type_name.as_str(), method.as_str())),
        _ => None,
    })
}

/// Le cas exact du ticket : `f.getCount().upper()` — `getCount()` retourne
/// `int`, `.upper()` n'existe sur aucun `int`.
#[test]
fn chained_call_on_int_return_is_rejected() {
    let src = r#"
        class Foo {
            public method getCount(): int {
                return 42
            }
        }
        function main(): int {
            var f:Foo = use Foo()
            var r:string = f.getCount().upper()
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert_eq!(method_call_on_non_class(&errors), Some(("int", "upper")), "errors = {:?}", errors);
}

#[test]
fn method_call_on_float_variable_is_rejected() {
    let src = r#"
        function main(): int {
            var x:float = 3.14
            x.foo()
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert_eq!(method_call_on_non_class(&errors), Some(("float", "foo")), "errors = {:?}", errors);
}

#[test]
fn method_call_on_bool_variable_is_rejected() {
    let src = r#"
        function main(): int {
            var b:bool = true
            b.foo()
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert_eq!(method_call_on_non_class(&errors), Some(("bool", "foo")), "errors = {:?}", errors);
}

#[test]
fn method_call_on_null_literal_is_rejected() {
    let src = r#"
        function main(): int {
            null.foo()
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert_eq!(method_call_on_non_class(&errors), Some(("null", "foo")), "errors = {:?}", errors);
}

/// Vérifié par reproduction (pas supposé, voir le ticket) : un appel de
/// fonction déclarée `: message<T>` utilisé directement comme récepteur est
/// bien atteignable ici, malgré les fortes restrictions de `message<T>`
/// ailleurs (jamais nommable en `var`/`scoped`/`consumed`).
#[test]
fn method_call_on_message_returning_call_is_rejected() {
    let src = r#"
        function gen(): message<int> {
            emit 1
            emit 2
        }
        function main(): int {
            gen().foo()
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert_eq!(method_call_on_non_class(&errors), Some(("message<int>", "foo")), "errors = {:?}", errors);
}

/// Vérifié par reproduction : les fonctions sont des valeurs de premier
/// ordre (`Type::Function`) et peuvent donc être stockées dans une variable
/// puis method-callées.
#[test]
fn method_call_on_function_value_is_rejected() {
    let src = r#"
        function add(a:int, b:int): int {
            return a + b
        }
        function main(): int {
            var f:Function<int(int, int)> = add
            f.foo()
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert_eq!(method_call_on_non_class(&errors), Some(("Function<int(int, int)>", "foo")), "errors = {:?}", errors);
}

/// Non-régression : `Type::Mixed` reste volontairement permissif — son
/// imprécision est un choix de langage assumé, pas un oubli comme les types
/// ci-dessus.
#[test]
fn method_call_on_mixed_variable_is_still_accepted() {
    let src = r#"
        function main(): int {
            var m:mixed = 5
            m.whatever()
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert!(method_call_on_non_class(&errors).is_none(), "errors = {:?}", errors);
}

/// Non-régression : les méthodes d'instance sucrées sur `string`/`array`
/// (voir docs/compilation-guide.md §6) continuent de fonctionner — le rejet
/// est spécifique aux types sans classe associée, pas à `String`/`Array`/`Map`
/// (qui ont chacun une entrée dédiée dans `type_class_name`).
#[test]
fn instance_sugar_methods_on_string_and_array_are_still_accepted() {
    let src = r#"
        function main(): int {
            var s:string = "hello"
            var u:string = s.upper()
            var a:array<int> = [1, 2, 3]
            a.push(4)
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert!(method_call_on_non_class(&errors).is_none(), "errors = {:?}", errors);
}

/// Non-régression : chaîner sur le retour d'une méthode qui retourne
/// RÉELLEMENT une instance de classe (pas un type primitif) continue de
/// fonctionner normalement.
#[test]
fn chained_call_on_class_return_is_still_accepted() {
    let src = r#"
        class Builder {
            public method withName(n:string): Builder {
                return self
            }
            public method build(): string {
                return "built"
            }
        }
        function main(): int {
            var b:Builder = use Builder()
            scoped r:string = b.withName("x").build()
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert!(method_call_on_non_class(&errors).is_none(), "errors = {:?}", errors);
}
