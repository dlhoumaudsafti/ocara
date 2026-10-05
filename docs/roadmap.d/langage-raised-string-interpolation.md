# `${e}` d'une chaîne levée affiche une adresse

## Constat

Trouvé en corrigeant [sema-builtin-parent-init-noop](sema-builtin-parent-init-noop.md) :

```ocara
try {
    raise "texte"
} on e {
    IO::writeln(`${e}`)    // affiche "4481120", attendu "texte"
}
```

Le handler générique `on e` type `e` en `mixed`. La valeur levée est une
chaîne littérale (`.rodata`, `TAG_STRING`), mais l'interpolation d'un `mixed`
l'affiche comme un entier. À comparer avec `${v}` sur un `mixed` construit
autrement (littéral de map, `JSON::decode`), qui affiche correctement les
chaînes : il faut voir si la valeur transmise par `__ocara_fail` au handler
garde bien son tag, ou si elle est stockée sous une forme non taggée.

`EBNF.md` §29.2 montre justement ce cas (`raise \`inconnu : ${e}\``).

## Priorité / Complexité

Haute (sortie fausse silencieuse) — **Simple à Légère**.

## Fichiers clés

`src/lower/stmt.d/` (lowering de `try`/`on`), `runtime/src/exception.rs`
(`__ocara_fail`), lowering des templates (`Expr::Template`).
