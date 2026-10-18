# Champ lu à travers un receveur `T|null` non vérifié : entier pris pour un pointeur (SIGSEGV)

Statut : **à corriger** — constaté le 2026-10-18 en écrivant le test de
stress de la collecte concurrente (`examples/34_concurrent_cycles.oc`).

## Constat

Un champ `int` lu à travers un receveur de type union `Node|null` sans
vérification préalable (`a.next.value`, `next:Node|null`) est traité comme
`mixed` sans être boxé : comparé à un `int`, il passe par
`__cmp_eq_strict`/`__cmp_ne_strict`, qui le lit comme un pointeur — SIGSEGV
dès que l'entier dépasse le seuil des pointeurs (petites valeurs : résultat
correct par chance). Même comportement avec le compilateur du 2026-10-17.

```ocara
architecture permissive
import ocara.IO

class Node {
    public property value:int
    public property next:Node|null = null
    init(value:int) {
        self.value = value
    }
}

function main(): int {
    var a:Node = use Node(1000005)
    var b:Node = use Node(1000005)
    a.next = b
    b.next = a
    if a.next.value not equal a.value {   // SIGSEGV
        IO::writeln("diff")
    }
    return 0
}
```

Avec `var n:Node|null = a.next` puis `if n is null {…} else { n.value … }`,
le résultat est correct.

## Correction attendue

Trancher : refuser l'accès à un membre d'un receveur `T|null` non vérifié
(erreur de sema, comme un accès sur `null`), ou bien typer le champ avec son
type déclaré (`int`) et vérifier `null` à l'exécution. Dans les deux cas, plus
de valeur brute dans un chemin `mixed`.

## Validation

La reproduction compile en erreur, ou affiche le bon résultat avec de grands
entiers.

## Priorité / Complexité

**Haute** (crash) — **Légère** à **Structurel** selon l'option retenue.
