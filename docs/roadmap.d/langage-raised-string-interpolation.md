# `${e}` d'une chaîne levée affichait une adresse — corrigé

## Constat

```ocara
try {
    raise "texte"
} on e {
    IO::writeln(`${e}`)    // affichait "4481120"
}
```

## Cause

Le lowering de `try`/`on` (`lower_try`,
`src/lower/stmt.d/statements.d/exceptions.rs`) déclarait la variable du
handler en `IrType::I64`. Le commentaire de la variante pour générateurs
(`message_gen.rs`) jugeait ce choix « cosmétique ». Or l'interpolation choisit
son formatage selon le type IR : en `I64`, la chaîne levée était affichée
comme un entier, c'est-à-dire son adresse.

## Correctif

Variable du handler déclarée `IrType::Ptr` (objet, ou `mixed` pour `on e`
sans filtre), dans `lower_try` et dans `lower_try_in_generator`. Les accès aux
champs (`e.message`) sont inchangés.

Test : `examples/tests/78_raised_string_interpolationTest.oc`.
