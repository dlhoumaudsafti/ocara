/// Tests unitaires — `property` de type ressource native (`Mutex`/`SQLite`/
/// `MySQL`/`MariaDB`/`HTTPRequest`/`HTTPResponse`) sur une classe utilisateur.
///
/// Jusqu'ici rejeté d'office (E29, `SemaError::ResourceField`, supprimé) :
/// aucun mécanisme n'existait pour fermer un tel champ. Désormais autorisé —
/// `__free_<Classe>` ferme le champ via son symbole runtime dédié (voir
/// `crate::lower::builder::class_ownership::classify_field`), et la classe
/// porteuse est traitée comme `OwnershipClass::Resource` PARTOUT où
/// l'échappement est vérifié (`crate::sema::scope::compute_resource_classes`/
/// `ownership_class_of`) : mêmes règles qu'une ressource nue (E18 : ne peut
/// pas s'échapper de son bloc `scoped`/`consumed` ; E28 : un `var`/`const`
/// doit être fermé manuellement ou prouvé non-échappant) — sans quoi deux
/// instances pourraient partager le même handle natif (`__clone_<Classe>` ne
/// clone jamais un champ ressource) et l'une le fermerait sous le nez de
/// l'autre. Un appel manuel `.close()`/`.destroy()` sur `self.<champ>` est
/// lui aussi rejeté (nouveau : `SemaError::ManualCloseOnResourceField`) —
/// `__free_<Classe>` le fermera de toute façon à la destruction de
/// l'instance, un appel en plus serait toujours une double fermeture.
///
/// Cas concret ayant motivé ce chantier :
/// `examples/advanced/tauri_httpserver/configs/Database.oc`
/// (`private property db:SQLite`, ouverte dans `init()`).
///
/// Repose sur un vrai pipeline lex → parse → symboles → typecheck (comme
/// `builtin_alias_resolution.rs`/`self_const_resolution.rs`).
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

fn has_unclosed_resource_var(errors: &[SemaError]) -> bool {
    errors.iter().any(|e| matches!(e, SemaError::UnclosedResourceVar { .. }))
}

fn has_manual_close_on_field(errors: &[SemaError]) -> bool {
    errors.iter().any(|e| matches!(e, SemaError::ManualCloseOnResourceField { .. }))
}

const DATABASE_CLASS: &str = r#"
    import ocara.SQLite

    class Database {
        private property db:SQLite

        init() {
            self.db = SQLite::open("./x.db")
        }

        public method migrate(): void {
            self.db.execute("CREATE TABLE x (id INTEGER)")
        }
    }
"#;

/// Une `property` de type ressource est maintenant acceptée à la
/// déclaration — plus aucune erreur de classe (E29 est supprimé).
#[test]
fn resource_property_declaration_is_accepted() {
    let src = format!("{DATABASE_CLASS}\nfunction main(): int {{ scoped d:Database = use Database() d.migrate() return 0 }}");
    let errors = check_errors(&src);
    assert!(errors.is_empty(), "errors = {:?}", errors);
}

/// Une instance `scoped` d'une classe contenant une ressource ne peut pas
/// s'échapper de son bloc (E18) — exactement comme une ressource nue :
/// sans ça, la copie « échappée » partagerait le même handle natif que
/// l'original (`__clone_<Classe>` ne duplique jamais un champ ressource).
#[test]
fn scoped_instance_cannot_escape_its_block() {
    let src = format!(
        "{DATABASE_CLASS}\nfunction main(): int {{ scoped d1:Database = use Database() var d2:Database = d1 return 0 }}"
    );
    let errors = check_errors(&src);
    assert!(has_resource_escape(&errors), "errors = {:?}", errors);
}

/// Un `var`/`const` prouvé non-échappant et jamais finalisé fuit son handle
/// natif pour toujours (E28) — `Database` n'a pas de méthode `close()` à
/// elle, la seule façon sûre reste `scoped`/`consumed`.
#[test]
fn non_escaping_var_instance_is_rejected_as_unclosed() {
    let src = format!(
        "{DATABASE_CLASS}\nfunction main(): int {{ var d:Database = use Database() d.migrate() return 0 }}"
    );
    let errors = check_errors(&src);
    assert!(has_unclosed_resource_var(&errors), "errors = {:?}", errors);
}

/// Appeler `.close()` manuellement sur `self.db` est rejeté : le champ est
/// déjà fermé automatiquement par `__free_Database` à la destruction de
/// l'instance, un appel manuel en plus serait toujours une double
/// fermeture — pas de suivi inter-méthodes possible pour distinguer un
/// usage « sûr ».
#[test]
fn manual_close_on_owned_resource_field_is_rejected() {
    let src = r#"
        import ocara.SQLite

        class Database {
            private property db:SQLite

            init() {
                self.db = SQLite::open("./x.db")
            }

            public method migrate(): void {
                self.db.execute("CREATE TABLE x (id INTEGER)")
                self.db.close()
            }
        }

        function main(): int {
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert!(has_manual_close_on_field(&errors), "errors = {:?}", errors);
}

/// Régression : une classe SANS champ ressource reste `OwnershipClass::Value`
/// — un `scoped`/`consumed` peut toujours « s'échapper » (cloné
/// automatiquement), comme avant ce chantier.
#[test]
fn ordinary_class_without_resource_field_still_clones_on_escape() {
    let src = r#"
        class Box {
            public property value:int
            init(v:int) {
                self.value = v
            }
        }

        function main(): int {
            scoped b1:Box = use Box(1)
            var b2:Box = b1
            return 0
        }
    "#;
    let errors = check_errors(src);
    assert!(!has_resource_escape(&errors), "errors = {:?}", errors);
}
