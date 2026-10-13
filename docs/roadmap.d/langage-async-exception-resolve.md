# `raise` dans une fonction `async` : non rattrapable autour de `resolve`

Statut : **à corriger** — constaté le 2026-10-13 (audit de la roadmap). Question
laissée « ouverte » par [langage-async-non-int-return-type-check](langage-async-non-int-return-type-check.md),
jamais tranchée ni documentée.

## Constat

```ocara
class Boom extends Exception { init(m:string) { parent::init(m) } }
async function work(): int { raise use Boom("x") }

var t:Resolvable<int> = work()
try {
    var v:int = resolve t
} on e is Boom {
    IO::writeln("rattrapé")
}
// « UNHANDLED EXCEPTION … Boom », exit 1
```

## Cause (à confirmer)

La tâche tourne sur un autre thread ; son `raise` remonte jusqu'au bas de la
pile de ce thread (aucun `try` actif) et termine le processus.
`__task_resolve` ne transmet pas l'exception au thread qui attend.

## Correction attendue

Capturer l'exception non rattrapée dans la tâche, la stocker dans le
`Resolvable`, et la relever dans le thread appelant au moment du `resolve`.
Documenter le comportement dans l'EBNF (§14.5).

## Validation

Test `examples/tests/` : exception rattrapée autour de `resolve`, exception
non rattrapée (code de sortie), exception d'une méthode `async` d'instance ;
`--leak-check` sans fuite.

## Priorité / Complexité

**Haute** (arrêt du programme sur un motif d'erreur ordinaire) — **Structurel**.
