/// Tests unitaires — `is_concrete_primitive_elem`/`concrete_elem_shape`
/// (docs/roadmap.d/qualite-tests-unitaires-critiques.md, point 2).

#[cfg(test)]
mod tests {
    use crate::lower::stmt::ownership::{is_concrete_primitive_elem, concrete_elem_shape};
    use crate::parsing::ast::Type;

    fn arr(inner: Type) -> Type {
        Type::Array(Box::new(inner))
    }

    fn map(inner: Type) -> Type {
        // La clé n'intervient jamais dans concrete_elem_shape — Int arbitraire ici.
        Type::Map(Box::new(Type::Int), Box::new(inner))
    }

    #[test]
    fn is_concrete_primitive_elem_true_for_int_float_bool_only() {
        assert!(is_concrete_primitive_elem(&Type::Int));
        assert!(is_concrete_primitive_elem(&Type::Float));
        assert!(is_concrete_primitive_elem(&Type::Bool));

        assert!(!is_concrete_primitive_elem(&Type::String));
        assert!(!is_concrete_primitive_elem(&Type::Mixed));
        assert!(!is_concrete_primitive_elem(&arr(Type::Int)));
        assert!(!is_concrete_primitive_elem(&map(Type::Int)));
        assert!(!is_concrete_primitive_elem(&Type::Named("Foo".into())));
    }

    #[test]
    fn concrete_elem_shape_single_level_primitive() {
        // array<int> : élément terminal, un seul niveau — chaîne vide (pas
        // de niveau d'imbrication SOUS le top-level).
        assert_eq!(concrete_elem_shape(&arr(Type::Int)), Some(String::new()));
        assert_eq!(concrete_elem_shape(&map(Type::Float)), Some(String::new()));
    }

    #[test]
    fn concrete_elem_shape_nested_depth_5_all_arrays() {
        // array<array<array<array<array<int>>>>> — même profondeur que le
        // filet de tests runtime (runtime/src/tests/boxing.rs, groupe 2),
        // pour la même raison : le bug historique corrigé un niveau
        // n'était pas couvert un cran plus profond.
        let depth5 = arr(arr(arr(arr(arr(Type::Int)))));
        assert_eq!(concrete_elem_shape(&depth5), Some("AAAA".to_string()));
    }

    #[test]
    fn concrete_elem_shape_mixed_array_and_map_nesting() {
        // array<map<string, array<int>>> — exerce le branchement 'A'/'M'
        // dans les deux sens, pas seulement des niveaux homogènes (voir
        // aussi runtime/src/tests/boxing.rs::free_concrete_mixed_array_of_maps_of_arrays,
        // même structure côté runtime).
        let ty = arr(Type::Map(Box::new(Type::String), Box::new(arr(Type::Int))));
        assert_eq!(concrete_elem_shape(&ty), Some("MA".to_string()));
    }

    #[test]
    fn concrete_elem_shape_none_as_soon_as_a_non_concrete_type_appears_at_any_depth() {
        // Rompt la chaîne concrète dès le premier niveau...
        assert_eq!(concrete_elem_shape(&arr(Type::String)), None);
        assert_eq!(concrete_elem_shape(&arr(Type::Mixed)), None);
        assert_eq!(concrete_elem_shape(&arr(Type::Named("Foo".into()))), None);

        // ... mais aussi à un niveau plus profond, PAS seulement au premier
        // niveau imbriqué — c'est exactement la classe de bug (retomber sur
        // le chemin générique dangereux au niveau N+1 alors que le niveau N
        // avait été traité correctement) documentée dans
        // docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md.
        assert_eq!(concrete_elem_shape(&arr(arr(Type::String))), None);
        assert_eq!(concrete_elem_shape(&arr(arr(arr(Type::Mixed)))), None);
    }

    #[test]
    fn concrete_elem_shape_none_on_a_non_container_type() {
        // concrete_elem_shape n'a de sens que pour Array/Map en argument —
        // tout autre type (y compris un primitif) n'est pas un conteneur.
        assert_eq!(concrete_elem_shape(&Type::Int), None);
        assert_eq!(concrete_elem_shape(&Type::String), None);
    }
}
