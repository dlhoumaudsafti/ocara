/// Tests unitaires — `rc_layout::is_counted` (docs/roadmap.d/memoire-refcount.md) :
/// une valeur comptée ne doit jamais pouvoir être un scalaire brut.

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::lower::builder::rc_layout::is_counted;
    use crate::parsing::ast::Type;

    fn objects() -> HashSet<String> {
        ["User".to_string()].into_iter().collect()
    }

    #[test]
    fn scalars_are_never_counted() {
        for ty in [Type::Int, Type::Float, Type::Bool, Type::Void, Type::Null] {
            assert!(!is_counted(&ty, &objects()), "{:?}", ty);
        }
    }

    #[test]
    fn heap_values_are_counted() {
        let arr = Type::Array(Box::new(Type::Int));
        let map = Type::Map(Box::new(Type::String), Box::new(Type::Mixed));
        for ty in [Type::String, Type::Mixed, arr, map, Type::Named("User".into())] {
            assert!(is_counted(&ty, &objects()), "{:?}", ty);
        }
    }

    #[test]
    fn unknown_named_types_are_not_counted() {
        assert!(!is_counted(&Type::Named("HTTPRequest".into()), &objects()));
    }

    /// Toute union est représentée comme un `mixed` (scalaires boxés) :
    /// comptée comme lui (docs/roadmap.d/memoire-boxing-temporaires-non-relaches.md).
    #[test]
    fn unions_are_counted_like_mixed() {
        let user_or_null = Type::Union(vec![Type::Named("User".into()), Type::Null]);
        let int_or_string = Type::Union(vec![Type::Int, Type::String]);
        let int_or_float = Type::Union(vec![Type::Int, Type::Float]);
        assert!(is_counted(&user_or_null, &objects()));
        assert!(is_counted(&int_or_string, &objects()));
        assert!(is_counted(&int_or_float, &objects()));
    }
}

/// Tests unitaires — sorties d'un corps `try`/gestionnaire `on`
/// (docs/roadmap.d/langage-try-controle-de-flux.md) : `return`, `break`,
/// `continue` rendent un code de sortie exécuté par la fonction englobante.
#[cfg(test)]
mod try_exit_tests {
    use crate::ir::inst::Inst;
    use crate::ir::module::IrModule;
    use crate::ir::types::IrType;
    use crate::lower::builder::program::lower_program;
    use crate::parsing::lexer::Lexer;
    use crate::parsing::parser::Parser;

    fn lower_src(src: &str) -> IrModule {
        let tokens = Lexer::new(src).tokenize().expect("lex");
        let program = Parser::new(tokens).parse_program().expect("parse");
        lower_program(&program, "<test>")
    }

    fn calls(module: &IrModule, func_name: &str, target: &str) -> bool {
        let func = module.functions.iter().find(|f| f.name == func_name).expect("fonction");
        func.blocks.iter().flat_map(|b| &b.insts).any(|i| matches!(i, Inst::Call { func, .. } if func == target))
    }

    #[test]
    fn try_body_and_handler_return_an_exit_code() {
        let module = lower_src(
            "function f(): string {\n\
                 try { return \"a\" } on e { return \"b\" }\n\
                 return \"c\"\n\
             }\n",
        );
        for name in ["__try_body_0", "__try_handler_0"] {
            let func = module.functions.iter().find(|f| f.name == name).expect("fonction");
            assert_eq!(func.ret_ty, IrType::I64, "{}", name);
            assert!(calls(&module, name, "__ocara_try_set_return"), "{}", name);
        }
        assert!(calls(&module, "f", "__ocara_try_take_return"));
    }

    #[test]
    fn break_in_try_body_leaves_the_body_with_a_code() {
        let module = lower_src(
            "function f(): int {\n\
                 var n:int = 0\n\
                 while true {\n\
                     try { n = n + 1\n if n equal 3 { break } } on e { }\n\
                 }\n\
                 return n\n\
             }\n",
        );
        let body = module.functions.iter().find(|f| f.name == "__try_body_0").expect("corps");
        let codes: Vec<i64> = body.blocks.iter().flat_map(|b| &b.insts)
            .filter_map(|i| match i { Inst::ConstInt { value, .. } => Some(*value), _ => None })
            .collect();
        assert!(codes.contains(&crate::lower::stmt::statements::try_exit_codes::TRY_BREAK));
    }
}
