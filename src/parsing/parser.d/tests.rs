/// Tests du parser

#[cfg(test)]
mod tests {
    use crate::parsing::ast::*;
    use crate::parsing::lexer::Lexer;
    use crate::parsing::parser::Parser;

    fn parse(src: &str) -> Program {
        let tokens = Lexer::new(src).tokenize().expect("lex error");
        Parser::new(tokens).parse_program().expect("parse error")
    }

    fn parse_expr(src: &str) -> Expr {
        let tokens = Lexer::new(src).tokenize().expect("lex error");
        Parser::new(tokens).parse_expr().expect("parse error")
    }

    // ── Import ───────────────────────────────────────────────────────────────

    #[test]
    fn test_import_simple() {
        let p = parse("import datas.User");
        assert_eq!(p.imports[0].path, vec!["datas", "User"]);
        assert_eq!(p.imports[0].alias, None);
    }

    #[test]
    fn test_import_alias() {
        let p = parse("import datas.User as UserData");
        assert_eq!(p.imports[0].alias, Some("UserData".into()));
    }

    // ── Constante ────────────────────────────────────────────────────────────

    #[test]
    fn test_const() {
        let p = parse("const TAX:float = 0.2");
        assert_eq!(p.consts[0].name, "TAX");
        assert_eq!(p.consts[0].ty, Type::Float);
        assert!(matches!(p.consts[0].value, Expr::Literal(Literal::Float(_), _)));
    }

    // ── Fonction ─────────────────────────────────────────────────────────────

    #[test]
    fn test_func_empty() {
        let p = parse("function main(): int { return 0 }");
        assert_eq!(p.functions[0].name, "main");
        assert_eq!(p.functions[0].ret_ty, Type::Int);
        assert_eq!(p.functions[0].params.len(), 0);
    }

    #[test]
    fn test_func_with_params() {
        let p = parse("function add(a:int, b:int): int { return 0 }");
        let params = &p.functions[0].params;
        assert_eq!(params[0].name, "a");
        assert_eq!(params[0].ty, Type::Int);
        assert_eq!(params[1].name, "b");
    }

    // ── Classe ───────────────────────────────────────────────────────────────

    #[test]
    fn test_class_basic() {
        let p = parse(r#"
            class User {
                private property name:string
                init(name:string) {}
                public method greet(): void {}
            }
        "#);
        let cls = &p.classes[0];
        assert_eq!(cls.name, "User");
        assert_eq!(cls.members.len(), 3);
    }

    #[test]
    fn test_class_extends_implements() {
        let p = parse("class A extends B implements C, D {}");
        let cls = &p.classes[0];
        assert_eq!(cls.extends, Some("B".into()));
        assert_eq!(cls.implements, vec!["C", "D"]);
    }

    // ── Interface ────────────────────────────────────────────────────────────

    #[test]
    fn test_interface() {
        let p = parse("interface Logger { method log(msg:string): void }");
        assert_eq!(p.interfaces[0].name, "Logger");
        assert_eq!(p.interfaces[0].methods[0].name, "log");
        assert_eq!(p.interfaces[0].methods[0].is_static, false);
    }

    // ── Interface : `wiring` (docs/roadmap.d/langage-interface-wiring.md) ────

    /// Un seul `wiring`, chemin pointé — `path` doit contenir chaque segment,
    /// `simple_name()` le DERNIER (celui qui compte pour la résolution).
    #[test]
    fn test_interface_wiring_single() {
        let p = parse("interface Repo { wiring infra.db.PostgresRepo\n method save(): void }");
        assert_eq!(p.interfaces[0].wirings.len(), 1);
        assert_eq!(p.interfaces[0].wirings[0].path, vec!["infra", "db", "PostgresRepo"]);
        assert_eq!(p.interfaces[0].wirings[0].simple_name(), "PostgresRepo");
    }

    /// `wiring` est répétable — l'ORDRE textuel de déclaration doit être
    /// préservé (significatif pour la résolution "nom nu" : premier gagnant).
    #[test]
    fn test_interface_wiring_multiple_preserves_order() {
        let p = parse(
            "interface Repo {\n\
                 wiring infra.db.PostgresRepo\n\
                 wiring infra.mem.InMemoryRepo\n\
                 method save(): void\n\
             }",
        );
        assert_eq!(p.interfaces[0].wirings.len(), 2);
        assert_eq!(p.interfaces[0].wirings[0].simple_name(), "PostgresRepo");
        assert_eq!(p.interfaces[0].wirings[1].simple_name(), "InMemoryRepo");
    }

    /// Une interface sans aucun `wiring` (cas le plus courant, non-régression) :
    /// `wirings` doit rester vide, jamais `None`/erreur.
    #[test]
    fn test_interface_without_wiring_has_empty_wirings() {
        let p = parse("interface Logger { method log(msg:string): void }");
        assert!(p.interfaces[0].wirings.is_empty());
    }

    /// `wiring` peut être mélangé aux méthodes dans n'importe quel ordre
    /// d'écriture (le ticket ne contraint pas où, dans le corps, `wiring`
    /// doit apparaître par rapport aux méthodes).
    #[test]
    fn test_interface_wiring_interleaved_with_methods() {
        let p = parse(
            "interface Repo {\n\
                 method save(): void\n\
                 wiring infra.PostgresRepo\n\
                 method load(): void\n\
             }",
        );
        assert_eq!(p.interfaces[0].methods.len(), 2);
        assert_eq!(p.interfaces[0].wirings.len(), 1);
    }

    /// `public method ...` — `public` est purement cosmétique dans une
    /// interface (toute méthode d'interface est de toute façon publique par
    /// nature) : doit parser sans erreur et sans affecter `is_static`.
    #[test]
    fn test_interface_method_public_modifier_is_cosmetic() {
        let p = parse("interface Repo { public method save(): void }");
        assert_eq!(p.interfaces[0].methods[0].name, "save");
        assert_eq!(p.interfaces[0].methods[0].is_static, false);
    }

    /// `public static method ...` — nécessaire pour qu'une interface puisse
    /// exiger un contrat STATIQUE (voir `wiring`, docs/roadmap.d/langage-interface-wiring.md) :
    /// `is_static` doit être `true`, `public` reste cosmétique.
    #[test]
    fn test_interface_method_static_modifier() {
        let p = parse("interface Repo { public static method create(): Repo }");
        assert_eq!(p.interfaces[0].methods[0].name, "create");
        assert_eq!(p.interfaces[0].methods[0].is_static, true);
    }

    /// `static method ...` (sans `public`) — les deux modificateurs sont
    /// indépendants l'un de l'autre.
    #[test]
    fn test_interface_method_static_modifier_without_public() {
        let p = parse("interface Repo { static method create(): Repo }");
        assert_eq!(p.interfaces[0].methods[0].is_static, true);
    }

    // ── Expressions ──────────────────────────────────────────────────────────

    #[test]
    fn test_expr_binary() {
        let expr = parse_expr("1 + 2 * 3");
        // doit respecter la précédence : 1 + (2 * 3)
        if let Expr::Binary { op: BinOp::Add, right, .. } = &expr {
            assert!(matches!(right.as_ref(), Expr::Binary { op: BinOp::Mul, .. }));
        } else {
            panic!("précédence incorrecte");
        }
    }

    #[test]
    fn test_expr_field_access() {
        let expr = parse_expr("user.age");
        assert!(matches!(expr, Expr::Field { .. }));
    }

    #[test]
    fn test_expr_range() {
        let expr = parse_expr("0..5");
        assert!(matches!(expr, Expr::Range { .. }));
    }

    #[test]
    fn test_expr_new() {
        let expr = parse_expr(r#"use User("Alice", 30)"#);
        if let Expr::New { class, args, .. } = expr {
            assert_eq!(class, "User");
            assert_eq!(args.len(), 2);
        } else {
            panic!("expected New");
        }
    }

    #[test]
    fn test_expr_static_call() {
        let expr = parse_expr("Math::abs(x)");
        assert!(matches!(expr, Expr::StaticCall { .. }));
    }

    // ── Types ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_type_array() {
        let p = parse("function foo(a:array<int>): int {}");
        assert_eq!(p.functions[0].params[0].ty, Type::Array(Box::new(Type::Int)));
    }

    #[test]
    fn test_type_map() {
        let p = parse("function foo(m:map<string,int>): int {}");
        assert_eq!(
            p.functions[0].params[0].ty,
            Type::Map(Box::new(Type::String), Box::new(Type::Int))
        );
    }

    #[test]
    fn test_type_qualified() {
        let p = parse("function foo(u:repository.User): void {}");
        assert_eq!(
            p.functions[0].params[0].ty,
            Type::Qualified(vec!["repository".into(), "User".into()])
        );
    }

    // ── Tableaux & Maps ───────────────────────────────────────────────────────

    #[test]
    fn test_array_literal() {
        let expr = parse_expr("[1, 2, 3]");
        if let Expr::Array { elements, .. } = expr {
            assert_eq!(elements.len(), 3);
        } else {
            panic!("expected Array");
        }
    }

    #[test]
    fn test_array_multidimensional() {
        let expr = parse_expr("[[1, 2], [3, 4]]");
        if let Expr::Array { elements, .. } = expr {
            assert_eq!(elements.len(), 2);
            assert!(matches!(elements[0], Expr::Array { .. }));
        } else {
            panic!("expected nested Array");
        }
    }

    #[test]
    fn test_map_literal() {
        let expr = parse_expr(r#"{"name": "Lucas", "age": 24}"#);
        if let Expr::Map { entries, .. } = expr {
            assert_eq!(entries.len(), 2);
        } else {
            panic!("expected Map");
        }
    }

    #[test]
    fn test_index_access() {
        let expr = parse_expr("arr[0]");
        assert!(matches!(expr, Expr::Index { .. }));
    }

    #[test]
    fn test_map_index_access() {
        let expr = parse_expr(r#"user["name"]"#);
        assert!(matches!(expr, Expr::Index { .. }));
    }
}
