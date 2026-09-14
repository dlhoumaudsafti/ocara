# Fuite d'échappement : `scoped`/`consumed` passée en argument

## ✅ Corrigé — diagnostic E26 (`ArgumentEscape`)

## Constat (historique)

Le modèle d'ownership (`scoped`/`consumed`) partait de l'hypothèse que rien ne peut créer un alias vers une valeur possédée (docs/EBNF.md §9.2). Cette hypothèse était fausse : `check_escape` (`src/sema/typecheck.rs`) n'était appelé que sur une affectation (`var y = x`, `y = x`) ou un `return x` — jamais sur un argument d'appel de fonction ou de constructeur.

## Reproduction (historique — désormais rejetée à la compilation)

```ocara
class Box {
    public property data:array<int>
    init(a:array<int>) { self.data = a }
}
function makeBox(): Box {
    scoped arr:array<int> = [111, 222, 333]
    var b:Box = use Box(arr)   // ❌ E26 : 'arr' sera libérée en fin de bloc
    return b
}
```

## Corrigé

Exactement la solution envisagée ci-dessous ("une vraie analyse d'échappement interprocédurale") : `src/sema/escape.rs` (nouveau module) détermine, pour chaque fonction/méthode/constructeur **utilisateur**, quels paramètres sont réellement retenus au-delà de l'appel (stockés dans un champ, retournés, capturés...). Un `scoped`/`consumed` (ressource : toujours ; `string`/`array`/`map`/classe utilisateur : seulement si le paramètre correspondant est prouvé retenu) passé à un tel paramètre est maintenant rejeté (**E26**), au lieu de corrompre silencieusement la mémoire. `Array::push(arr, x)` (builtin, jamais résolu par ce module) reste autorisé — le risque de casser ce sucre, identifié ci-dessous, est bien évité.

Ce chantier a été mené **conjointement** avec [memoire-strategie-var](memoire-strategie-var.md) (même analyse interprocédurale, réutilisée pour décider si un `var` peut être libéré automatiquement) — voir cette fiche pour le détail complet de l'implémentation, des tests, et des limites assumées (imprécision toujours du côté sûr).

## Fichiers clés

Voir [memoire-strategie-var.md](memoire-strategie-var.md#fichiers-clés) — même implémentation, `src/sema/error.rs` (`ArgumentEscape`), `docs/EBNF.md` §9.2, `docs/diagnostics.md` (E26).
