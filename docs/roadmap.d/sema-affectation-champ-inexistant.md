# Affectation à un champ inexistant acceptée — écriture à l'offset 0, SIGSEGV

Statut : **à corriger** — constaté le 2026-10-22 en préparant
[langage-map-acces-pointe](langage-map-acces-pointe.md).

## Constat

La **lecture** d'un champ inexistant est refusée (`field 'A' not found in
class 'Map'`), mais l'**affectation** compile, quel que soit le type de
l'objet, et écrit la valeur à l'offset 0 du bloc :

```ocara
var truc:map<string, int> = {"A": 1, "B": 2}
truc.A = 1000000                 // compile
for i in 0 until 1000 { truc[`k${i}`] = i }
// munmap_chunk(): invalid pointer — abandon (exit 134)

var s:string = "ab"
s.x = 5                          // compile, puis SIGSEGV (exit 139)

var a:array<int> = [1, 2]
a.x = 1000000                    // compile, puis SIGSEGV après des push

var p:P = use P()                // classe avec un seul champ `x`
p.nope = 5                       // compile : écrase `p.x` en silence
IO::writeln(`${p.x}`)            // 5
```

`truc.A = 3` seul ne plante pas tout de suite : la valeur est ignorée
(`truc["A"]` vaut toujours 1) et la structure interne de la map est
corrompue. `m.size = 5` (nom d'une méthode) compile de la même façon.

## Cause

`src/sema/typecheck.rs`, `Stmt::Assign`, cible `Expr::Field` : quand
`lookup_field_owner` ne trouve pas le champ, aucune erreur n'est émise. Le
lowering (`src/lower/stmt.d/statements.d/assignments.rs`) émet alors un
`SetField` à l'offset 0.

## Correction attendue

Même diagnostic qu'en lecture (`SemaError::FieldNotFound`) pour une
affectation, une affectation composée (`+=`…) et `++`/`--` sur un champ
inconnu, pour toute classe (utilisateur ou builtin `Map`/`Array`/`String`).
`mixed` reste hors de ce correctif (permissif par choix, voir E63 dans
[sema-acces-champ-index-non-classe](sema-acces-champ-index-non-classe.md),
et la question de conception dans
[langage-map-acces-pointe](langage-map-acces-pointe.md)).

Tests : sema (`src/sema/tests/`) pour chaque forme de cible, exemple de
régression qui vérifie le diagnostic.
