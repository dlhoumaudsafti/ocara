# Types des arguments jamais vérifiés à l'appel

Statut : **corrigé** le 2026-10-13 — erreur **E62** (voir `docs/diagnostics.md`).

## Constat

La sema vérifie le **nombre** d'arguments d'un appel (`WrongArgCount`), jamais
leur **type**. Aucune erreur, aucun avertissement :

```ocara
architecture permissive
import ocara.IO
import ocara.Math

class P {
    public property x:int
    init(x:int) { self.x = x }
}
class C {
    public method add(a:int, b:int): int { return a + b }
    public static method sadd(a:int, b:int): int { return a + b }
}
function add(a:int, b:int): int { return a + b }
function takeP(p:P): int { return p.x }

function main(): int {
    IO::writeln(add("1", 2).toStr())        // affiche 10051650
    add(a: "un", b: 2)                      // argument nommé
    takeP("pas un P")                        // objet attendu
    use C().add("deux", 3)                   // méthode d'instance
    C::sadd("deux", 3)                       // méthode statique
    use P("texte")                           // constructeur
    Math::abs("texte")                       // builtin
    return 0
}
```

À l'exécution, la valeur est réinterprétée telle quelle : l'adresse d'une
chaîne devient un entier (`add("1", 2)` → `10051650`), une chaîne passée
pour un objet fait lire des champs inexistants (comportement indéfini,
crash possible).

## À faire

- Comparer le type de chaque argument (positionnel et nommé, variadic
  compris) au type du paramètre, avec la règle de compatibilité déjà
  utilisée pour les affectations et les `return` (`types_compat` : unions,
  `null` pour `T|null`, sous-classes et interfaces, `mixed`), pour : fonctions
  libres, méthodes d'instance et statiques (héritées, via interface,
  `parent::`), constructeurs (`use X(…)`, `parent::init`), closures appelées
  via une variable `Function<…>`, builtins (signatures de
  `src/builtins/`).
- Diagnostic sur l'argument fautif : « argument `b` de `add` : `string`
  attendu `int` ».
- Valeurs par défaut des paramètres vérifiées de la même façon.

## Risque

Activer la vérification peut révéler des appels incorrects dans les
exemples et la bibliothèque : les corriger un par un (régression complète).

## Correction

`src/sema/arg_types.rs` : `check_call_args` compare chaque argument (liste
positionnelle déjà résolue pour un appel nommé, valeurs par défaut
comprises) au type de son paramètre avec `types_compat`, pour les fonctions
libres, méthodes d'instance (sucre d'instance des builtins : receveur
retiré), méthodes via interface, méthodes statiques, `parent::m(…)` et
`parent::init(…)` (arité aussi, jamais contrôlée jusque-là), constructeurs
(`T` d'un `generic` substitué par l'argument de type) et sucre JSON ; un
élément de `variadic<T>` est comparé à `T` (un `T[]` est aussi accepté).
`check_param_defaults` vérifie la valeur par défaut à la déclaration. Les
appels via une variable `Function<…>` et les méthodes d'un générique
instancié étaient déjà vérifiés (E01).

Pas de conversion implicite `int` → `float` (même règle que `var f:float =
3`). Diagnostic : `argument 'b' of 'add': expected type 'int', found
'string'`, sur l'argument fautif.

Révélé par le contrôle, corrigé :

- `examples/builtins/math.oc` : `Math::sqrt(int)` affichait `Distance (3,4)
  = 0.0000…0111` au lieu de `5` → `squares.toFloat()`.
- Signatures builtin imprécises (`Function<void()>` pour « n'importe quelle
  fonction ») : `HTTPServer.route`/`routeError` →
  `Function<int(HTTPServerRequest)>`, `HTMLComponent.register` →
  `Function<string(map<string,mixed>)>`, `Map::forEach` →
  `Function<void(mixed, mixed)>`.

Tests : `src/sema/tests/arg_types.rs`.

## Priorité / Complexité

**Haute** (résultat faux silencieux, comportement indéfini) — sema
(`src/sema/typecheck.rs`, `named_args.rs`), signatures builtin.
