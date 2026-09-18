/// Tests unitaires — `core::alias_resolve` (résolution des alias d'import
/// `import X as Y` pour les symboles utilisateur). Voir
/// docs/roadmap.d/langage-alias-classe-utilisateur-heritage-casse.md pour le
/// bug corrigé : renommer le symbole importé (ancien comportement) cassait
/// la résolution partout où un AUTRE fichier référence le même symbole par
/// son vrai nom. Ces tests fixent le comportement de la passe de
/// substitution elle-même (Type/Expr/Stmt), indépendamment du pipeline
/// complet — voir `examples/project/tests/AliasClassInheritanceTest.oc` pour
/// une vérification de bout en bout (compilation + exécution réelle).
use std::collections::HashMap;
use crate::core::alias_resolve::{compute_aliases, resolve_aliases};
use crate::parsing::ast::*;
use crate::parsing::token::Span;

fn span() -> Span { Span::new(0, 0) }
fn ident(name: &str) -> Expr { Expr::Ident(name.to_string(), span()) }

fn aliases(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

fn import(path: &[&str], alias: Option<&str>) -> ImportDecl {
    ImportDecl {
        path: path.iter().map(|s| s.to_string()).collect(),
        file_path: None,
        alias: alias.map(|s| s.to_string()),
        span: span(),
    }
}

// ── compute_aliases ─────────────────────────────────────────────────────────

/// `import configs.Server as HTTP` → alias "HTTP" mappé vers le DERNIER
/// segment du chemin ("Server"), pas le chemin complet.
#[test]
fn compute_aliases_maps_alias_to_last_path_segment() {
    let imports = vec![import(&["configs", "Server"], Some("HTTP"))];
    let map = compute_aliases(&imports);
    assert_eq!(map.get("HTTP"), Some(&"Server".to_string()));
    assert_eq!(map.len(), 1);
}

/// Un import SANS alias ne produit aucune entrée — rien à réécrire.
#[test]
fn compute_aliases_ignores_imports_without_alias() {
    let imports = vec![import(&["configs", "Server"], None)];
    assert!(compute_aliases(&imports).is_empty());
}

/// `import X as X` (alias identique au nom réel, cas dégénéré mais possible
/// syntaxiquement) ne doit produire aucune substitution — un no-op, pas une
/// entrée `"X" -> "X"` qui ne changerait rien de toute façon mais autant
/// l'exclure explicitement.
#[test]
fn compute_aliases_ignores_alias_identical_to_real_name() {
    let imports = vec![import(&["Server"], Some("Server"))];
    assert!(compute_aliases(&imports).is_empty());
}

// ── resolve_aliases : types de paramètre / extends / implements ────────────

/// Le cas exact du bug : un paramètre de méthode typé par le nom RÉEL
/// ("Server") doit rester intact quand l'alias mappé est un AUTRE nom
/// ("HTTP") — ne rien réécrire par erreur dans le mauvais sens.
#[test]
fn resolve_aliases_leaves_unrelated_type_names_untouched() {
    let mut program = Program::new();
    program.classes.push(ClassDecl {
        name: "Caller".into(), extends: None, modules: vec![], implements: vec![],
        members: vec![ClassMember::Constructor {
            params: vec![Param { name: "server".into(), ty: Type::Named("Server".into()), default_value: None, is_variadic: false, span: span() }],
            body: Block { stmts: vec![], span: span() },
            span: span(),
        }],
        span: span(),
    });
    resolve_aliases(&mut program, &aliases(&[("HTTP", "Server")]));
    let ClassMember::Constructor { params, .. } = &program.classes[0].members[0] else { panic!() };
    assert_eq!(params[0].ty, Type::Named("Server".into()));
}

/// `class Child extends Alias` → `extends` doit être réécrit vers le vrai nom.
#[test]
fn resolve_aliases_rewrites_extends() {
    let mut program = Program::new();
    program.classes.push(ClassDecl {
        name: "Child".into(), extends: Some("MyChild".into()), modules: vec![], implements: vec![],
        members: vec![], span: span(),
    });
    resolve_aliases(&mut program, &aliases(&[("MyChild", "RealChild")]));
    assert_eq!(program.classes[0].extends, Some("RealChild".into()));
}

/// `class Foo implements Alias` → chaque entrée de `implements` réécrite.
#[test]
fn resolve_aliases_rewrites_implements() {
    let mut program = Program::new();
    program.classes.push(ClassDecl {
        name: "Foo".into(), extends: None, modules: vec![], implements: vec!["MyIface".into()],
        members: vec![], span: span(),
    });
    resolve_aliases(&mut program, &aliases(&[("MyIface", "RealIface")]));
    assert_eq!(program.classes[0].implements, vec!["RealIface".to_string()]);
}

// ── resolve_aliases : expressions ───────────────────────────────────────────

/// `use Alias()` (`Expr::New`) → le champ `class` doit être réécrit.
#[test]
fn resolve_aliases_rewrites_expr_new() {
    let mut program = Program::new();
    program.functions.push(FuncDecl {
        name: "main".into(), params: vec![], ret_ty: Type::Void, is_async: false, span: span(),
        body: Block {
            stmts: vec![Stmt::Expr(Expr::New { class: "MyChild".into(), type_args: vec![], args: vec![], span: span() })],
            span: span(),
        },
    });
    resolve_aliases(&mut program, &aliases(&[("MyChild", "RealChild")]));
    let Stmt::Expr(Expr::New { class, .. }) = &program.functions[0].body.stmts[0] else { panic!() };
    assert_eq!(class, "RealChild");
}

/// `Alias::method()` (`Expr::StaticCall`) → le champ `class` doit être
/// réécrit, `method` ne doit JAMAIS être touché (ce n'est pas un nom de
/// symbole importé).
#[test]
fn resolve_aliases_rewrites_static_call_class_only() {
    let mut program = Program::new();
    program.functions.push(FuncDecl {
        name: "main".into(), params: vec![], ret_ty: Type::Void, is_async: false, span: span(),
        body: Block {
            stmts: vec![Stmt::Expr(Expr::StaticCall { class: "MyChild".into(), method: "greet".into(), args: vec![], span: span() })],
            span: span(),
        },
    });
    resolve_aliases(&mut program, &aliases(&[("MyChild", "RealChild"), ("greet", "shouldNeverMatchAMethodName")]));
    let Stmt::Expr(Expr::StaticCall { class, method, .. }) = &program.functions[0].body.stmts[0] else { panic!() };
    assert_eq!(class, "RealChild");
    assert_eq!(method, "greet", "le nom de MÉTHODE ne doit jamais être réécrit par la table d'alias");
}

/// L'alias doit être réécrit récursivement à travers les arguments d'un
/// appel — pas seulement au premier niveau de l'AST.
#[test]
fn resolve_aliases_recurses_into_nested_call_arguments() {
    let mut program = Program::new();
    program.functions.push(FuncDecl {
        name: "main".into(), params: vec![], ret_ty: Type::Void, is_async: false, span: span(),
        body: Block {
            stmts: vec![Stmt::Expr(Expr::Call {
                callee: Box::new(ident("takes")),
                args: vec![Expr::New { class: "MyChild".into(), type_args: vec![], args: vec![], span: span() }],
                span: span(),
            })],
            span: span(),
        },
    });
    resolve_aliases(&mut program, &aliases(&[("MyChild", "RealChild")]));
    let Stmt::Expr(Expr::Call { args, .. }) = &program.functions[0].body.stmts[0] else { panic!() };
    let Expr::New { class, .. } = &args[0] else { panic!() };
    assert_eq!(class, "RealChild");
}

// ── resolve_aliases : `on e is X` ───────────────────────────────────────────

/// `on e is Alias` (`OnClause.class_filter`) → doit être réécrit vers le
/// vrai nom, sinon le filtre ne correspondrait plus jamais à rien après que
/// la classe elle-même a cessé d'exister sous l'alias.
#[test]
fn resolve_aliases_rewrites_on_clause_filter() {
    let mut program = Program::new();
    program.functions.push(FuncDecl {
        name: "main".into(), params: vec![], ret_ty: Type::Void, is_async: false, span: span(),
        body: Block {
            stmts: vec![Stmt::Try {
                body: Block { stmts: vec![], span: span() },
                handlers: vec![OnClause {
                    binding: "e".into(),
                    class_filter: Some("MyException".into()),
                    body: Block { stmts: vec![], span: span() },
                    span: span(),
                }],
                span: span(),
            }],
            span: span(),
        },
    });
    resolve_aliases(&mut program, &aliases(&[("MyException", "RealException")]));
    let Stmt::Try { handlers, .. } = &program.functions[0].body.stmts[0] else { panic!() };
    assert_eq!(handlers[0].class_filter, Some("RealException".into()));
}

// ── resolve_aliases : no-op sur une table vide ──────────────────────────────

/// Table d'alias vide (fichier sans aucun `as`) → programme totalement
/// inchangé, aucun coût ni effet de bord.
#[test]
fn resolve_aliases_is_noop_with_empty_alias_table() {
    let mut program = Program::new();
    program.classes.push(ClassDecl {
        name: "Foo".into(), extends: Some("Bar".into()), modules: vec![], implements: vec![],
        members: vec![], span: span(),
    });
    let before = program.clone();
    resolve_aliases(&mut program, &HashMap::new());
    assert_eq!(program, before);
}
