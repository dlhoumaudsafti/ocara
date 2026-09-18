# `use Classe(...).méthode()` chaîné perd la valeur de retour de la méthode

## Constat

Découvert en écrivant `examples/tests/52_class_resource_property_destructorTest.oc` (chantier du destructeur de champ ressource, voir [langage-destructeur-champ-ressource](langage-destructeur-champ-ressource.md)) — **sans aucun rapport avec ce chantier-là**, reproduit avec une classe qui ne contient même pas de ressource :

```ocara
import ocara.IO

class Bar {
    init() {
    }
    public method getIt(): int {
        return 777
    }
}

function main(): int {
    var r1:int = use Bar().getIt()
    IO::writeln(`r1 = ${r1}`)   // affiche 0, pas 777

    scoped b:Bar = use Bar()
    var r2:int = b.getIt()
    IO::writeln(`r2 = ${r2}`)   // affiche 777 — correct
    return 0
}
```

`use Classe(args).méthode()` **chaîné en une seule expression** retourne toujours `0` au lieu de la vraie valeur de retour de `méthode()` — que le constructeur ait des arguments ou non (`use Bar(5).getIt()` et `use Bar().getIt()` reproduisent tous les deux). Séparer `use`/l'appel en deux étapes (`scoped b:Bar = use Bar()` puis `b.getIt()`) fonctionne correctement.

**Compile sans erreur ni avertissement** — résultat silencieusement faux, pas un rejet à la compilation : le genre de bug le plus dangereux pour ce langage (voir la philosophie de priorisation en tête de `docs/roadmap.md`).

## Impact réel sur du code existant

`examples/advanced/tauri_httpserver/controllers/HomeController.oc` fait exactement `scoped visits:int = use Database().recordVisit()` — cette valeur vaut donc probablement `0` à chaque rendu de page, pas le vrai compteur de visites. Non vérifié en conditions réelles (nécessiterait de lancer le serveur HTTP complet), mais le repro minimal ci-dessus est sans ambiguïté sur la mécanique en cause.

## Piste non explorée (hypothèse, à vérifier)

`use Classe(args)` se parse en un simple `Expr::New` (même nœud AST que si `new` existait comme mot-clé séparé — voir `src/parsing/parser.d/expressions.rs` autour de `TokenKind::Use`), donc `use Bar().getIt()` est un `Expr::Call` dont le récepteur (`object`) est directement un `Expr::New`, pas un `Expr::Ident`. Le chemin de lowering générique pour un appel de méthode d'instance (`src/lower/expr.d/lower.rs`, bloc `Expr::Field { object, field, .. }` de `Expr::Call`) a l'air de fonctionner pour CE seul appel — reste à vérifier si un mécanisme de libération automatique du temporaire créé par `Expr::New` (l'instance `Bar` jetable, jamais liée à une variable nommée) réutilise ou écrase la valeur de retour de `getIt()` avant que l'appelant ne la lise — non confirmé, juste l'hypothèse la plus probable au vu du symptôme (la valeur devient exactement `0`, pas une valeur aléatoire/un plantage).

## Priorité / Complexité

**Priorité Haute** — résultat silencieusement faux, aucun rejet à la compilation, motif de priorisation explicite de cette roadmap (voir son en-tête). Bloque potentiellement `HomeController.oc` de l'exemple `tauri_httpserver`.
**Complexité : non évaluée** — cause racine non confirmée, à investiguer avant d'estimer.

## Fichiers clés

`src/parsing/parser.d/expressions.rs` (`TokenKind::Use` → `Expr::New`), `src/lower/expr.d/lower.rs` (lowering d'un appel de méthode dont le récepteur est directement `Expr::New`, pas un `Expr::Ident`), `examples/advanced/tauri_httpserver/controllers/HomeController.oc` (impact réel suspecté, non vérifié en conditions réelles).
