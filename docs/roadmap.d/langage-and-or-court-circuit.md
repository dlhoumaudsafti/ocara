# `and` / `or` sans court-circuit ; condition `mixed` testée comme un pointeur

Audit du 2026-10-19. Reproduit sur le binaire courant.

## Constat

1. **`and` et `or` évaluent toujours leurs deux opérandes.**
   ```ocara
   var n:Node|null = null
   if n not equal null and n.v equal 1 { … }   // lève "Cannot access 'v' on null"
   IO::writeln(false and side("B"))            // affiche "Bfalse"
   ```
   `d not equal 0 and 10 / d equal 2` lève « Division by zero ». La garde
   « non null puis champ », très courante, ne protège rien. Non documenté ;
   les langages courants court-circuitent.
   `src/lower/expr.d/lower.rs` (~l.1494) émet `Inst::And`/`Inst::Or` après
   avoir abaissé les deux côtés ; `logical.rs` les traduit en `band`/`bor`.
2. **Condition `mixed` (ou `bool|…`) : le pointeur de la boîte est testé, pas
   le booléen.**
   ```ocara
   var f:mixed = false
   if f { … }                         // branche prise
   var mm:map<string, mixed> = {"active": false}
   if mm["active"] { … }              // branche prise (maps issues de JSON)
   ```
   `not t`, `x and t`, `while f` sont faux de la même manière. Une
   affectation `var b:bool = f` déballe correctement.

## Piste

Abaisser `a and b` en `if a { b } else { false }` (blocs + phi), idem `or` ;
déballer (`__unbox_bool`) un opérande `mixed`/union dans `if`, `while`,
`not`, `and`, `or`. Documenter le court-circuit dans l'EBNF.

## Priorité / Complexité

**Haute** (valeurs fausses, exceptions inattendues) — **Légère**.

## Fichiers clés

`src/lower/expr.d/lower.rs`, `src/codegen/emit.d/instructions.d/logical.rs`,
`src/lower/stmt.d/statements.d/control_flow.rs`, `docs/EBNF.md` (opérateurs).
