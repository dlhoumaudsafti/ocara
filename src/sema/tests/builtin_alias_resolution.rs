/// Tests unitaires — `import ocara.X as Y` cassait la résolution de plusieurs
/// comportements natifs spéciaux pour `HTTPRequest`/`HTTPResponse` dans
/// `TypeChecker` (voir `SymbolTable::local_name_for_builtin`).
///
/// `register_import` enregistre une classe builtin SOUS UNE SEULE clé dans
/// la table des symboles : l'alias s'il y en a un, sinon le nom canonique —
/// jamais les deux à la fois. Or `typecheck.rs` comparait plusieurs
/// comportements natifs (carve-out d'échappement de ressource pour les
/// fonctions statiques `HTTPRequest::*`, redirection `HTTPResponse` →
/// `HTTPRequest` pour trouver les méthodes d'instance comme `.ok()`) contre
/// le nom canonique EN DUR ("HTTPRequest"). Dès que l'import était aliasé
/// (`import ocara.HTTPRequest as Request`), ces comparaisons ratent
/// silencieusement, rejetant à tort du code par ailleurs valide — reproduit
/// en pratique dans `examples/advanced/tauri_httpserver` (`Request::ok(res)`
/// puis `res.ok()`).
///
/// Repose sur un vrai pipeline lex → parse → symboles → typecheck (pas un
/// graphe d'AST synthétique comme `escape.rs`) : le bug dépend de
/// l'enregistrement RÉEL de l'alias par `SymbolTable::register_import`,
/// impossible à simuler sans reproduire ce pipeline.
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

fn has_resource_escape(errors: &[SemaError]) -> bool {
    errors.iter().any(|e| matches!(e, SemaError::ResourceEscape { .. }))
}

fn has_field_not_found(errors: &[SemaError]) -> bool {
    errors.iter().any(|e| matches!(e, SemaError::FieldNotFound { .. }))
}

/// `Request::ok(res)` où `Request` est un alias de `HTTPRequest` : le
/// carve-out d'échappement pour les fonctions statiques `HTTPRequest::*`
/// (dont le handle ressource est passé en argument par construction, voir
/// `check_argument_escape`) comparait `resolved_class == "HTTPRequest"` en
/// dur — avec l'alias, `resolved_class` valait `"Request"`, donc `res` était
/// rejeté à tort comme un échappement de ressource.
#[test]
fn aliased_httprequest_static_call_does_not_flag_resource_escape() {
    let src = r#"
        import ocara.HTTPRequest as Request

        function main(): int {
            scoped res:HTTPResponse = Request::get("http://localhost/health")
            if Request::ok(res) {
                return 0
            }
            return 1
        }
    "#;
    let errors = check_errors(src);
    assert!(!has_resource_escape(&errors), "errors = {:?}", errors);
}

/// `res.ok()` (sucre d'instance) quand `HTTPRequest` est importé sous alias
/// ET `HTTPResponse` est explicitement importé : la redirection
/// `HTTPResponse` → `HTTPRequest` (seule classe à porter réellement les
/// méthodes `status`/`body`/`ok`/...) cherchait la classe sous le nom
/// canonique `"HTTPRequest"`, introuvable dès que l'import est aliasé —
/// "field 'ok' not found in class 'HTTPResponse'" sur du code par ailleurs
/// valide.
#[test]
fn aliased_httprequest_instance_sugar_resolves_on_httpresponse() {
    let src = r#"
        import ocara.HTTPRequest as Request
        import ocara.HTTPResponse

        function main(): int {
            scoped res:HTTPResponse = Request::get("http://localhost/health")
            if res.ok() {
                return 0
            }
            return 1
        }
    "#;
    let errors = check_errors(src);
    assert!(!has_field_not_found(&errors), "errors = {:?}", errors);
}

/// Régression : un VRAI échappement de ressource doit rester rejeté — le
/// carve-out ne doit s'appliquer qu'aux appels `HTTPRequest::*` légitimes
/// (alias compris), jamais désactiver la vérification en général.
#[test]
fn real_resource_escape_is_still_rejected() {
    let src = r#"
        import ocara.Mutex

        function main(): int {
            scoped m:Mutex = use Mutex()
            var leaked:Mutex = m
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert!(has_resource_escape(&errors), "errors = {:?}", errors);
}

/// Régression : passer une ressource en argument à un appel statique
/// `Classe::méthode(res)` qui N'EST PAS `HTTPRequest` (ici une classe
/// utilisateur) doit rester rejeté — `is_http_request_call` ne doit être vrai
/// que pour `HTTPRequest` (alias compris), jamais pour un autre callee.
#[test]
fn resource_argument_to_non_httprequest_static_call_is_still_rejected() {
    let src = r#"
        import ocara.SQLite

        class Helper {
            public static method useDb(db:SQLite): void {
            }
        }

        function main(): int {
            scoped db:SQLite = SQLite::open("./x.db")
            Helper::useDb(db)
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert!(has_resource_escape(&errors), "errors = {:?}", errors);
}
