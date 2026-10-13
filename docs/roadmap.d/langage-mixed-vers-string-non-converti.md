# Valeur `mixed` contenant un entier utilisée comme `string` — SIGSEGV

Statut : **à corriger** — constaté le 2026-10-13 (audit de la roadmap).

## Constat

```ocara
// route "/v/<id:int>"
var s:string = req.param("id")    // param() : mixed, ici un int
req.respond(200, s)               // le serveur se termine (exit 139)
```

Variante sans crash mais fausse : `String::len(s)` rend `0` alors que `${s}`
affiche `42`. La fiche [stdlib-httpserver-route-params](stdlib-httpserver-route-params.md)
qualifiait ce SIGSEGV de « comportement attendu » : c'est un défaut — un
programme qui passe la sema ne doit pas planter sur une conversion de type.

## Cause

`src/lower/stmt.d/statements.d/helpers.rs` (~l.19-31) ne déballe un `mixed`
que vers `int`/`float` ; vers `string`, la valeur est transmise telle quelle et
`ptr_to_str` déréférence l'entier comme une adresse.

## Correction attendue

Au choix (à trancher) :
- conversion vérifiée à l'exécution (`mixed` → `string` : chaîne telle quelle,
  autre type → exception claire ou conversion textuelle) ;
- ou refus à la compilation d'une affectation `mixed` → `string` sans
  conversion explicite.

## Validation

Test `examples/tests/` (affectation, argument, retour), cas HTTPServer dans
`examples/builtins/httpserver.sh` ; mise à jour de la fiche route-params.

## Priorité / Complexité

**Haute** (crash d'un serveur sur une requête ordinaire) — **Structurel**.
