# Constante de classe à valeur négative : nom du symbole lu à la place de la valeur

## Constat (reproduit)

```ocara
class T {
    public const ZERO:int = -273
}
IO::writeln(`${T::ZERO}`)                // affiche "T__ZERO"
IO::writeln(`${0 greater or equal T::ZERO}`)   // false
```

`-273` est un `Expr::Unary { Neg, Literal }`, pas un `Expr::Literal` : la
collecte des constantes inlinables (`module.class_consts`,
`src/lower/builder.d/program.rs`) et l'émission de la globale
(`ClassMember::Const` dans `src/lower/builder.d/classes.rs`, `bytes =
vec![]` pour tout autre forme qu'un littéral) l'ignorent — la lecture
retombe sur l'adresse/le nom du symbole `T__ZERO`.

Échec de `examples/tests/23_static_methodTest.oc` (`Temperature::is_valid(0)`,
`ABSOLUTE_ZERO = -273`) **masqué jusqu'ici** par ocaraunit.

## Correction

`Expr::const_literal` (`src/parsing/ast.d/const_fold.rs`) évalue à la
compilation littéraux, `-x`, `not x` et `+ - * / %` entre littéraux ; utilisé
pour l'inlining (`module.class_consts`) et la globale émise. Toute autre
valeur est désormais rejetée en sema (E55, `docs/diagnostics.md`) au lieu de
produire un symbole vide. Tests unitaires dans `const_fold.rs`, plus
`23_static_methodTest` et `70_typed_container_literalsTest`.

## Priorité / Complexité

**Haute** (valeur silencieusement fausse) — **Simple**.
