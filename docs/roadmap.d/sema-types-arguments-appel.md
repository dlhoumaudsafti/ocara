# Types des arguments jamais vérifiés à l'appel

Statut : **à corriger** — constaté le 2026-10-13.

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

## Priorité / Complexité

**Haute** (résultat faux silencieux, comportement indéfini) — sema
(`src/sema/typecheck.rs`, `named_args.rs`), signatures builtin.
