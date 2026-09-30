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

## À faire

- En sema, sur `objet.champ` (lecture) et `objet.champ = ...` (affectation) :
  `private` → accessible uniquement depuis la classe déclarante ;
  `protected` → depuis la classe déclarante et ses descendantes.
- Nouveau diagnostic dédié.
- **Risque** : des exemples existants profitent peut-être de ce trou (accès
  externe à un champ `private`) — lancer la régression complète et migrer
  ce qui casse avant d'activer le contrôle.

## Priorité / Complexité

**Haute** (le mot-clé de visibilité est silencieusement sans effet) —
**Légère** (contrôle local en sema, mais migration d'exemples possible).

## Fichiers clés

`src/sema/typecheck.rs` (`Expr::Field`, `Stmt::Assign`),
`src/sema/symbols.d/types.rs` (`FieldInfo.vis`), `src/sema/error.rs`,
`docs/diagnostics.md`.
