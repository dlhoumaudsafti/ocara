# Appel chaîné cassé à partir de 3 niveaux — `a.b().c().d()` (et non `a.b().c()`)

## Constat (vérifié indépendamment)

Trouvé en marge de [langage-chained-call-on-free-function-result](langage-chained-call-on-free-function-result.md)
en testant des variantes du correctif. **Général et préexistant**, sans
rapport avec les fonctions libres ni les interfaces — reproduit avec une
chaîne de méthodes d'instance PURE, aucune fonction libre en jeu :

```ocara
class Wrapper {
    public property inner:Circle
    init(c:Circle) { self.inner = c }
    public method getCircle(): Circle { return self.inner }
}
class Circle {
    public method shapeName(): string { return "circle" }
}
function main(): int {
    var w:Wrapper = use Wrapper(use Circle(2.0))
    IO::writeln(w.getCircle().shapeName().upper())   // affiche "null"
    return 0
}
```

`w.getCircle().shapeName()` (2 niveaux) fonctionne déjà correctement — le
échec commence au 3ème niveau (`.upper()` sur le résultat).

## Cause (localisée, pas encore corrigée)

Toute la famille de résolution "classe du récepteur d'un appel chaîné"
(`src/lower/expr.d/lower.rs`, `typeinfer.rs`, `helpers.rs::resolve_chained_field_class`)
ne recurse qu'**un seul niveau** : le cas `Expr::Call { callee: Expr::Field
{ object: inner_obj, .. } }` résout `inner_obj` uniquement s'il est lui-même
`Expr::Ident` (variable nommée) — jamais s'il est LUI-MÊME un
`Expr::Call`/`Expr::Field` imbriqué. Une vraie correction demanderait de
transformer ces blocs de `match` répétés (un par fichier, quasi-identiques)
en une seule fonction récursive partagée `resolve_receiver_class(expr) ->
Option<String>`, appelable à n'importe quelle profondeur — pas juste
ajouter un nouveau cas de plus au même patron non-récursif.

## Priorité / Complexité

**Haute** (résultat silencieusement faux, pas un crash) — **Structurel**
(refactor en fonction récursive partagée, pas un cas de plus dans un
`match` existant ; à répliquer identiquement dans au moins 3 fichiers
aujourd'hui dupliqués).

## Fichiers clés

`src/lower/expr.d/lower.rs`, `src/lower/expr.d/typeinfer.rs`,
`src/lower/expr.d/helpers.rs` (`resolve_chained_field_class`).
