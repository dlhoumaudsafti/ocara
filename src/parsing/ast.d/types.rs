/// Types et visibilité pour l'AST Ocara

// ─────────────────────────────────────────────────────────────────────────────
// Types Ocara v1.0.0
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    Mixed,
    Void,
    Null,
    /// Type nommé (classe, interface, alias d'import)
    Named(String),
    /// Type qualifié : `repository.User`
    Qualified(Vec<String>),
    /// `Type[]`
    Array(Box<Type>),
    /// `map<K, V>`
    Map(Box<Type>, Box<Type>),
    /// `message<T>` — générateur (voir docs/roadmap.d/langage-emit-iterable.md).
    /// Valable UNIQUEMENT comme type de retour déclaré d'une fonction/méthode
    /// contenant au moins un `emit` — jamais comme type de paramètre, jamais
    /// nommable (`var`/`scoped`/`consumed`), jamais un type de premier ordre
    /// ailleurs. Vérifié par la sema (`src/sema/typecheck.rs`).
    Message(Box<Type>),
    /// Type générique avec arguments : `List<int>`, `Cache<string, User>`
    Generic {
        name: String,
        args: Vec<Type>,
    },
    /// `T | U | ...` — type union
    Union(Vec<Type>),
    /// Référence à une fonction ou méthode statique (premier ordre) :
    /// Syntaxe : `Function<ReturnType(ParamType1, ParamType2, ...)>`
    Function {
        ret_ty: Box<Type>,
        param_tys: Vec<Type>,
    },
}

/// Premier variant `Type::Named` (classe utilisateur) d'un type union
/// (`Classe|null`, `Classe|Autre`...) — `None` si `ty` n'est pas un
/// `Type::Union` ou n'en contient aucun. Un seul niveau de dépliage suffit :
/// un `Type::Union` ne peut pas être imbriqué dans un autre (voir
/// `Parser::parse_type`, src/parsing/parser.d/types_parsing.rs — seul
/// `parse_type_base`, qui ne reconnaît jamais lui-même un `|`, alimente les
/// variantes d'un `Type::Union`), contrairement à `Type::Array`/`Type::Map`
/// (imbriquables à profondeur arbitraire, voir `container_elem_type` dans
/// src/lower/expr.d/helpers.rs).
///
/// Placé ici (plutôt que dans un seul des modules `lower::*` qui l'utilisent)
/// car ses appelants sont dispersés dans les TROIS sous-arbres du lowering
/// (`builder.d`, `expr.d`, `stmt.d`) — chaque site qui lie un nouvel
/// identifiant à un type déclaré/résolu doit enregistrer sa classe pour
/// l'accès aux champs/méthodes d'instance (`var_class` dans `LowerBuilder`) :
/// `lower_var`/`lower_const` (src/lower/stmt.d/statements.d/variables.rs),
/// la variable de boucle `for x in array<Classe|null>`
/// (src/lower/stmt.d/statements.d/loops.rs), un paramètre de fonction/méthode
/// (src/lower/builder.d/functions.rs) ou de closure nameless
/// (src/lower/expr.d/nameless.rs).
///
/// Bug historique que ce partage corrige : seul `lower_var` dépliait
/// `Type::Union` avant ce correctif — TOUS les autres sites listés
/// ci-dessus (dont `lower_const`, le cas le plus courant :
/// `const x:Classe|null = Repo::find()`) ne le faisaient pas, donc `x`
/// n'avait AUCUNE entrée `var_class`. Tout accès de champ (`Expr::Field`)
/// sur `x` — même après narrowing (`if x is null { return }`) — résolvait
/// alors `class_name = None`, ce qui retombe sur `offset = 0` pour
/// N'IMPORTE QUEL champ (voir `field_offset`, src/lower/expr.d/helpers.rs) :
/// silencieusement, TOUJOURS la valeur du premier champ déclaré, quel que
/// soit le champ réellement demandé. Confirmé par reproduction (le
/// contournement — réassigner vers une variable de type concret AVANT
/// d'accéder aux champs — fonctionnait, ce qui pointait déjà vers une
/// résolution de classe manquante plutôt qu'un bug d'offset lui-même). Voir
/// docs/roadmap.d/langage-union-class-null-field-access.md.
pub fn union_named_class(ty: &Type) -> Option<String> {
    if let Type::Union(variants) = ty {
        variants.iter().find_map(|v| match v {
            Type::Named(n) => Some(n.clone()),
            _ => None,
        })
    } else {
        None
    }
}

/// `ty` est-il, directement ou via un union (`Classe|null`), le nom d'une
/// classe utilisateur ? Combine le cas direct (`Type::Named`) et
/// `union_named_class` (le cas union) — les deux formes sous lesquelles un
/// site de binding (variable de boucle `for x in array<T>`, valeur d'un
/// `for k => v in map<K,V>`...) peut avoir besoin de la même réponse : "quelle
/// classe dois-je enregistrer dans `var_class` pour ce type élément ?".
pub fn resolved_named_class(ty: &Type) -> Option<String> {
    match ty {
        Type::Named(n) => Some(n.clone()),
        _ => union_named_class(ty),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(n: &str) -> Type { Type::Named(n.to_string()) }

    #[test]
    fn union_named_class_finds_named_variant() {
        let ty = Type::Union(vec![named("Foo"), Type::Null]);
        assert_eq!(union_named_class(&ty), Some("Foo".to_string()));
    }

    #[test]
    fn union_named_class_none_when_no_named_variant() {
        // `int|null` — aucune classe à enregistrer.
        let ty = Type::Union(vec![Type::Int, Type::Null]);
        assert_eq!(union_named_class(&ty), None);
    }

    #[test]
    fn union_named_class_none_on_non_union_type() {
        // Un `Type::Named` DIRECT (pas union) n'est pas de son ressort —
        // c'est `resolved_named_class` qui couvre les deux cas.
        assert_eq!(union_named_class(&named("Foo")), None);
        assert_eq!(union_named_class(&Type::Int), None);
    }

    #[test]
    fn union_named_class_first_named_variant_wins() {
        // `Foo|Bar|null` — le premier Named trouvé (Foo), pas le dernier ;
        // comportement hérité de `lower_var` avant cette factorisation,
        // inchangé ici.
        let ty = Type::Union(vec![named("Foo"), named("Bar"), Type::Null]);
        assert_eq!(union_named_class(&ty), Some("Foo".to_string()));
    }

    #[test]
    fn resolved_named_class_covers_direct_and_union_forms() {
        assert_eq!(resolved_named_class(&named("Foo")), Some("Foo".to_string()));
        assert_eq!(
            resolved_named_class(&Type::Union(vec![named("Foo"), Type::Null])),
            Some("Foo".to_string())
        );
        assert_eq!(resolved_named_class(&Type::Int), None);
        assert_eq!(resolved_named_class(&Type::Array(Box::new(Type::Int))), None);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Visibilité
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    Public,
    Private,
    Protected,
}

// ─────────────────────────────────────────────────────────────────────────────
// Paramètre de type générique
// ─────────────────────────────────────────────────────────────────────────────

use crate::parsing::token::Span;

/// Paramètre de type générique (ex: T, K, V = string)
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    pub name:    String,
    /// Valeur par défaut optionnelle
    pub default: Option<Type>,
    pub span:    Span,
}
