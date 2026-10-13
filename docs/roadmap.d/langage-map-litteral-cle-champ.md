# Littéral map `{obj.champ: valeur}` refusé par le parseur

Statut : **à corriger** — constaté le 2026-10-13 (audit de la roadmap).

## Constat

```ocara
var m:map<int,string> = {c.ret: "x"}
// error: expected type, found LitString("x")
```

Contournement : `{(c.ret): "x"}`. Contraire à l'EBNF
(`MapEntry ::= Expression ":" Expression`, ~l.1074). Rend aussi faux le
deuxième exemple de [reflexion-declarations-conditionnelles-when](reflexion-declarations-conditionnelles-when.md).

## Cause

`src/parsing/parser.d/expressions.rs` (~l.351-356) : après `.champ`, le
parseur avale un `:` suivi d'un type (annotation de la forme `user.age:int`).

## Correction attendue

Ne pas interpréter `: Type` après un accès de champ quand on est dans une
entrée de littéral map (ou restreindre cette annotation aux contextes qui
l'admettent réellement, à vérifier dans l'EBNF).

## Validation

Test parser (`src/parsing/tests`) + test `examples/tests/` ; relecture de
l'exemple de la fiche `when`.

## Priorité / Complexité

**Haute** (programme valide refusé) — **Légère**.
