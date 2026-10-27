# Affectation à un champ inexistant acceptée — écriture à l'offset 0, SIGSEGV

Statut : **corrigé** le 2026-10-22 (constaté le même jour en préparant
la notation pointée sur les maps, `docs/EBNF.md` §7.4).

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

## Correction

`src/sema/unknown_field.rs` (`TypeChecker::unknown_field`) : un champ absent
d'une classe (chaîne d'héritage comprise), d'un `String`/`Array`/`Map` ou
d'une instance de `generic` (parent compris) donne `FieldNotFound` (E08) à
l'affectation ; un nom de méthode n'y est pas un champ. `+=` (désucré en
`a.f = a.f + v`) et `++` étaient déjà refusés par la lecture : une seule
erreur par champ. Une classe opaque (import non résolu), un parent de
`generic` inconnu et `mixed` restent permissifs.

La lecture d'un champ absent d'une instance de `generic` (`b.nope`), elle
aussi acceptée et lue à l'offset 0, est refusée de la même façon.

Tests : `src/sema/tests/unknown_field.rs`. Documentation : E08 dans
`docs/diagnostics.md`.
