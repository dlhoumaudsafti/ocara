# Visibilité des champs (`private`/`protected`) jamais vérifiée à l'accès

## Constat (reproduit)

```ocara
class C {
    protected property y:int
    private property z:int
    init() {
        self.y = 2
        self.z = 3
    }
}

function main(): int {
    var c:C = use C()
    IO::writeln(`${c.y} ${c.z}`)   // compile et affiche "2 3"
    return 0
}
```

Aucune erreur : `Expr::Field` (`src/sema/typecheck.rs`) ne consulte jamais
`FieldInfo.vis` (enregistré par la table des symboles) — un champ `private`
ou `protected` est lisible ET modifiable depuis n'importe où. Même chose
pour un champ `protected` d'un `struct` (§16.7 de l'EBNF), qui n'a donc
aujourd'hui aucun effet.

Découvert en implémentant `struct` — préexistant, sans rapport.

## Correction

Nouveau diagnostic **E54** (`SemaError::FieldNotAccessible`,
`src/sema/field_visibility.rs`), appliqué en lecture (`Expr::Field`, donc
aussi `++`/`--`) et en affectation (`Stmt::Assign` sur un champ, qui
n'inférait jusqu'ici que l'objet). La classe déclarante est retrouvée par
`SymbolTable::lookup_field_owner` ; `protected` s'appuie sur
`class_matches` (chaîne `extends`). Un champ de module est composé dans la
classe utilisatrice, qui en est donc la déclarante.

Aucun exemple ne profitait du trou : régression complète et tous les
projets de `examples/` (advanced, project, from, mods, generics) vérifiés
sans nouvelle erreur. Tests : `src/sema/tests/field_visibility.rs`.

## Priorité / Complexité

**Haute** (le mot-clé de visibilité est silencieusement sans effet) —
**Légère** (contrôle local en sema, mais migration d'exemples possible).

## Fichiers clés

`src/sema/typecheck.rs` (`Expr::Field`, `Stmt::Assign`),
`src/sema/symbols.d/types.rs` (`FieldInfo.vis`), `src/sema/error.rs`,
`docs/diagnostics.md`.
