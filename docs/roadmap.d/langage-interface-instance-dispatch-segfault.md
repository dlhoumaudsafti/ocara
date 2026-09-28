# SIGSEGV : appeler une méthode d'instance sur une variable typée par une interface

## Constat

Trouvé en marge du chantier [langage-interface-wiring](langage-interface-wiring.md)
(confirmé orthogonal à `wiring` lui-même, voir plus bas) — un cas de
polymorphisme d'interface pourtant déjà documenté comme fonctionnel
(`implements`, existant bien avant `wiring`) segfault dès qu'on appelle une
méthode d'instance à travers le type interface :

```ocara
import ocara.IO

interface Shape {
    method area(): float
}

class Circle implements Shape {
    public property r:float
    init(r:float) { self.r = r }
    public method area(): float {
        return 3.14 * self.r * self.r
    }
}

function main(): int {
    const c:Circle = use Circle(2.0)
    var s:Shape = c        // affectation valide : Circle implémente Shape
    IO::writeln(s.area())  // SIGSEGV
    return 0
}
```

Reproduit indépendamment (`ocara build` + exécution directe, pas seulement
via la suite de tests) : compilation réussie, segfault à l'exécution sur
`s.area()`.

## Piste de cause (à vérifier, pas encore confirmée par un correctif)

Le dispatch dynamique par identité de classe (`class_dispatcher_name`,
`src/lower/builder.d/class_dispatch.rs`) ne connaît que l'héritage de
CLASSE (`classes_with_subclasses`), jamais les interfaces. Un mécanisme
distinct existe déjà (`generate_interface_dispatchers`,
`src/lower/builder.d/interfaces.rs`, introduit par le chantier `wiring`
pour les méthodes STATIQUES d'une interface avec `wiring`) mais ne semble
pas branché sur le chemin d'appel d'une méthode D'INSTANCE via une variable
simplement typée par l'interface (sans aucun `wiring` en jeu ici — polymorphisme
classique). À vérifier en profondeur avant de coder quoi que ce soit : cette
hypothèse vient du chantier `wiring`, où elle n'était qu'un sous-produit de
l'investigation, pas l'objet principal.

## Pourquoi ce n'est pas dans le ticket `wiring`

`wiring` ne produit jamais de binding interface-typé : sa substitution
(alias ou nom nu, en position construction/appel statique) résout TOUJOURS
vers un type CONCRET avant que le reste du pipeline ne s'en mêle — elle ne
traverse donc jamais ce chemin cassé. Le bug touche uniquement le
polymorphisme d'interface ORDINAIRE (`var x:UneInterface = uneInstanceConcrete`),
qui existait déjà avant `wiring` et n'a pas de rapport avec lui.

## Priorité / Complexité

**Priorité Haute** — un SIGSEGV sur un usage basique et déjà documenté
d'une fonctionnalité existante (`implements` + polymorphisme d'instance)
correspond exactement à la définition de cette section : bloque la
fiabilité du langage. Complexité non évaluée avant investigation — touche
la génération du dispatch d'instance (`src/lower/builder.d/class_dispatch.rs`),
probablement en l'étendant pour reconnaître aussi les interfaces (pas
seulement l'héritage de classe), et vérifier son interaction avec
`generate_interface_dispatchers` (`src/lower/builder.d/interfaces.rs`) sans
dupliquer un second mécanisme de dispatch.

## Fichiers clés

`src/lower/builder.d/class_dispatch.rs` (`class_dispatcher_name`,
`classes_with_subclasses`), `src/lower/builder.d/interfaces.rs`
(`generate_interface_dispatchers`, mécanisme existant mais distinct, ajouté
par [langage-interface-wiring](langage-interface-wiring.md)).
