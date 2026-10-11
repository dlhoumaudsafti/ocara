# Appel de méthode sur une valeur typée par une interface : non vérifié

Statut : **corrigé** (2026-10-09) — constaté via le serveur de langage.

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

## Correction

`typecheck.rs` : un appel sur une valeur typée par une interface est
vérifié comme un appel sur une classe — méthode cherchée dans l'interface,
arguments nommés (`resolve_named_call`), arité (`WrongArgCount`),
méthode `static` appelée sur une instance (`StaticOnInstance`), type de
retour (`call_ret_ty`, `async` compris). Aucun exemple ni test existant
n'était concerné. Tests : `src/sema/tests/interface_call.rs`.

Reste commun aux deux chemins (classe et interface) : le **type** de chaque
argument n'est pas comparé à celui du paramètre, seule l'arité l'est.
