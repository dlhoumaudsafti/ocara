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

## Piste

Évaluer à la compilation les expressions constantes simples (au minimum
`-<littéral>`) pour les constantes de classe, comme pour les constantes
globales si celles-ci le font déjà ; sinon, rejeter explicitement en sema
toute valeur de constante non évaluable plutôt que produire un symbole vide.

## Priorité / Complexité

**Haute** (valeur silencieusement fausse) — **Simple**.
