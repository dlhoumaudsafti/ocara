# Fuite d'échappement : `scoped`/`consumed` passée en argument

## Constat

Le modèle d'ownership (`scoped`/`consumed`) part de l'hypothèse que rien ne peut créer un alias vers une valeur possédée (docs/EBNF.md:1198, src/lower/stmt.d/ownership.rs:24). Cette hypothèse est fausse : `check_escape` (src/sema/typecheck.rs:651-674) n'est appelé que sur une affectation (`var y = x`, `y = x`) ou un `return x` — jamais sur un argument d'appel de fonction ou de constructeur.

## Reproduction

Un objet dont le constructeur stocke un paramètre `scoped` dans un champ récupère un pointeur vers une mémoire qui est libérée (`__value_free`) à la fin du bloc appelant. Reproduit et exécuté : le champ contient ensuite des valeurs aléatoires (mémoire déjà `dealloc()`ée), sans aucune erreur ni avertissement à la compilation (`--check` passe).

```ocara
class Box {
    public property data:array<int>
    init(a:array<int>) { self.data = a }
}
function makeBox(): Box {
    scoped arr:array<int> = [111, 222, 333]
    var b:Box = use Box(arr)   // arr passée par alias, jamais clonée
    return b
}                              // fin de bloc : __array_free(arr) ici
```

→ lecture de `b.data` après retour : valeurs corrompues.

## Portée du problème

Touche tout pattern OOP courant (constructeur qui garde un paramètre), toute fonction qui stocke un argument `scoped`/`consumed` dans une structure qui survit à l'appel. Variante identique pour une capture de closure/thread (src/lower/expr.d/lower.rs:1340-1372) — voir [memoire-concurrence-threads](memoire-concurrence-threads.md).

## Pourquoi ce n'est pas un simple correctif

Un clone systématique aux frontières d'appel casserait des cas déjà voulus (ex. `Array::push(arr, x)` — src/sema/typecheck.rs:638-650, un appel qui modifie `arr` en place). Il faut soit une vraie analyse d'échappement interprocédurale, soit une distinction explicite dans la signature des fonctions entre "emprunte" et "prend possession".

## Fichiers clés

`src/sema/typecheck.rs` (`check_escape`, ses 3 seuls points d'appel), `src/lower/stmt.d/ownership.rs`, `docs/EBNF.md` §9.2.
