# Appel de méthode sur une valeur typée par une interface : non vérifié

Statut : **à faire** — constaté le 2026-10-09 (serveur de langage).

## Constat

Dans `src/sema/typecheck.rs`, un appel `valeur.methode(...)` n'est vérifié
que si le type de la valeur est une **classe** (`lookup_class`). Pour une
valeur typée par une **interface**, la branche est sautée : les arguments ne
sont pas comparés aux paramètres déclarés, et l'appel est typé `mixed`.

```ocara
interface Speaker {
    public method speak(): string
}
class Animal implements Speaker {
    public method speak(): string { return "..." }
}

function main(): int {
    var a:Animal = use Animal()
    var s:Speaker = a
    var n:int = s.speak()   // accepté : string affectée à un int
    var m:int = a.speak()   // refusé : expected type 'int', found 'string'
    return n + m
}
```

Conséquences : une valeur `string` rangée dans un `int` sans diagnostic
(affichage de l'adresse, calculs faux), arité et types des arguments jamais
vérifiés à travers une interface.

## À faire

- Résoudre la méthode dans l'interface (`lookup_interface(...).methods`),
  vérifier arité et types des arguments, typer l'appel avec son retour
  (`call_ret_ty`, `async` compris), arguments nommés via `resolve_named_call`.
- Vérifier l'effet sur les exemples et tests existants (des programmes
  aujourd'hui acceptés peuvent devenir refusés, à juste titre).
- Le serveur de langage indexe déjà ces appels (références, renommage).

## Priorité / Complexité

**Haute** (sûreté du typage) — **Légère** (une branche de la sema, plus les
tests).
