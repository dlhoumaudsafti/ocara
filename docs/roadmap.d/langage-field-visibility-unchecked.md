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

## Reste à faire — audit du 2026-10-13

Les champs sont vérifiés (E54, `src/sema/tests/field_visibility.rs`), mais **ni les méthodes ni les constantes de classe** :

```ocara
k.secret()          // méthode private : compile, affiche 2
K::ps()             // protected static : compile, affiche 7
Config::SECRET      // constante private : compile, affiche abc
```

Contraire à l'EBNF §9 et §16.3 (« les règles de visibilité s'appliquent normalement »), en contradiction avec §17 qui reconnaît que `FuncSig` n'a aucun champ de visibilité. Une classe peut aussi implémenter une méthode d'interface en `private` et rester appelable de partout. Correction : porter la visibilité dans `FuncSig` et les constantes, même règle qu'E54. **Haute (programme invalide accepté) — Légère.**

**Corrigé le 2026-10-13** : `SemaError::MemberNotAccessible` (E54) pour les méthodes (instance, statiques, `parent::`) et les constantes de classe, même règle que les champs (`src/sema/field_visibility.rs`, table `method_vis` des classes du programme, modules compris). Tests : `src/sema/tests/member_visibility.rs`. La visibilité déclarée dans une interface reste ignorée (limitation volontaire).
