# SEGFAULT : une fermeture créée dans un bloc `if`/`while`/`for` perd la promotion heap de la variable qu'elle capture, dès qu'on la relit après le bloc

## Constat

Découvert en testant le correctif de [langage-nested-closure-recapture](langage-nested-closure-recapture.md) : contrairement à ce que ce dernier ticket pouvait laisser penser, le problème ne se limite pas à l'imbrication de fermetures — **une fermeture à un seul niveau, si elle est simplement créée à l'intérieur d'un bloc `if`/`while`/`for`, plante en SEGFAULT dès que la variable qu'elle capture est relue APRÈS le bloc.**

### Reproduction minimale

```ocara
import ocara.IO

function main(): int {
    var counter:int = 0
    if true {
        var f:Function<void()> = nameless(): void { counter = counter + 1 }
        f()
    }
    IO::writeln(`${counter}`)   // SEGFAULT ici
    return 0
}
```

`./ocara repro.oc -o repro && ./repro` → `Erreur de segmentation (core dumped)`.

Variante boucle (`while`, pas de crash mais résultat silencieusement faux — 0 au lieu de 5) :
```ocara
var counter:int = 0
var i:int = 0
while i smaller 5 {
    var f:Function<void()> = nameless(): void { counter = counter + 1 }
    f()
    i = i + 1
}
IO::writeln(`${counter}`)   // affiche 0, pas 5 — aucun crash, juste faux
```

Le fait qu'un cas plante et l'autre se contente d'un résultat faux suggère un accès mémoire invalide dont la conséquence dépend du contenu du tas à cet endroit précis (comportement non défini, pas une différence de nature entre les deux cas).

## Root cause identifiée précisément (via `ocara --dump`)

Sur le repro `if`, l'IR généré pour `main` :
```
bb0: Alloca { dest: Value(0), ty: I64 }        // counter, stack, AVANT le if
     ...
bb1 (corps du if) :
     Call { dest: Some(Value(4)), func: "__alloc_locked_cell", args: [] }
     ...
     builder.locals["counter"] redirigée vers Value(4) (voir
     src/lower/expr.d/lower.rs:1538, `builder.locals.insert(cap_name, (heap_ptr, ...))`)
bb3 (après le if, `IO::writeln(counter)`) :
     Call { dest: Some(Value(21)), func: "__locked_cell_get", args: [Value(0)] }
                                                  ^^^^^^^ toujours Value(0), le
                                                  slot stack D'ORIGINE, jamais
                                                  redirigé vers Value(4)
```

`bb3` appelle `__locked_cell_get` (qui attend un pointeur de cellule verrouillée, alloué par `__alloc_locked_cell`) avec `Value(0)` — le pointeur de la `Alloca` stack ORIGINALE de `counter`, jamais promue à cet endroit. Confusion de représentation : `__locked_cell_get` déréférence `Value(0)` comme si c'était une cellule verrouillée valide, alors que c'est un slot stack ordinaire — accès mémoire invalide, SEGFAULT.

Cause : la redirection `builder.locals.insert("counter", (heap_ptr, ...))` faite lors de la création de la fermeture (§`Expr::Nameless`, `src/lower/expr.d/lower.rs:1538`) a lieu PENDANT le lowering du bloc `then` du `if` — mais cette mise à jour de `builder.locals` **ne survit pas à la sortie du bloc** : au moment de lower le code APRÈS le `if` (`bb3`), `builder.locals["counter"]` a été restaurée (ou n'a jamais été propagée) à son entrée d'AVANT le bloc (`Value(0)`, jamais promue). Suggère que le lowering d'un bloc (`if`/`while`/`for` — voir `src/lower/stmt.d/block.rs`, `lower_if`/`lower_while`/`lower_for_in`) sauvegarde/restaure `builder.locals` à ses frontières (probablement pour que les déclarations LOCALES AU bloc ne fuient pas vers l'extérieur, ce qui est correct) — mais applique cette même restauration, À TORT, aux entrées qui existaient DÉJÀ avant le bloc et qui ont seulement été REDIRIGÉES (promotion heap), pas nouvellement déclarées.

**Non testé, mais plausible** : le même mécanisme est probablement en cause pour `Thread::spawn`/`Mutex.withLock` (n'importe quel appel prenant une closure) dès que l'appel a lieu à l'intérieur d'un bloc conditionnel/boucle — pas seulement `nameless` assignée à une variable comme dans le repro.

## Portée

Plus sévère que [langage-nested-closure-recapture](langage-nested-closure-recapture.md) : ne nécessite AUCUNE imbrication de fermeture, juste une fermeture ordinaire à un seul niveau, créée dans n'importe quel bloc conditionnel/boucle, dont la variable capturée est relue après. C'est un patron extrêmement courant (`if condition { ... var f = nameless(){...}; f() ... }`, ou plus simplement toute closure créée conditionnellement). Risque réel de SEGFAULT en production sur du code par ailleurs parfaitement raisonnable.

## Ce qui est demandé

Pas de correctif tenté ici (diagnostic seulement, zone « Dangereuse » — lowering des blocs, partagé par tout le langage). Piste à évaluer : dans `lower_if`/`lower_while`/`lower_for_in`/`lower_for_map` (`src/lower/stmt.d/`), s'assurer que toute redirection de `builder.locals` pour une variable qui existait AVANT l'entrée du bloc (promotion heap déclenchée par une capture de fermeture à l'intérieur du bloc) est bien répercutée dans la table `locals` de l'appelant après la sortie du bloc — alors que les déclarations NOUVELLES faites dans le bloc, elles, doivent bien rester locales à ce bloc (ne pas fuiter). Distinction à faire entre "nouvelle entrée" et "entrée existante redirigée".

Ajouter un test de régression `.oc` dès que corrigé (aucun aujourd'hui).

## Priorité / Complexité

**Priorité Haute** — SEGFAULT (pas juste une valeur fausse) sur un patron de code très courant, aucune configuration exotique requise. **Complexité : Dangereuse** — touche le lowering des blocs, partagé par `if`/`while`/`for`/`switch`/`try`, risque de casser des centaines de programmes fonctionnels si mal corrigé ; budget de tests de non-régression large impératif.

## Fichiers clés

`src/lower/stmt.d/block.rs`, `src/lower/stmt.d/statements.rs` (`lower_if`, `lower_while`, `lower_for_in`, `lower_for_map`), `src/lower/expr.d/lower.rs` (`Expr::Nameless`, la redirection `builder.locals.insert` à corriger de l'autre côté), [langage-nested-closure-recapture](langage-nested-closure-recapture.md) (bug connexe, mécanisme de capture).
