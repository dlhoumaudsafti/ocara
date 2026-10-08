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

    #[test]
    fn nullable_union_follows_its_member() {
        let user_or_null = Type::Union(vec![Type::Named("User".into()), Type::Null]);
        let int_or_string = Type::Union(vec![Type::Int, Type::String]);
        assert!(is_counted(&user_or_null, &objects()));
        assert!(!is_counted(&int_or_string, &objects()));
    }
}
