# Primitif boxé vers un paramètre `mixed`/union ou un conteneur — jamais relâché

Statut : **corrigé** le 2026-10-13 (constaté le même jour par l'audit de la roadmap).

## Constat

```ocara
function f(x:mixed): void {}
// 2 000 000 appels f(1.5) : 64,7 Mo ; --leak-check : 2000000 × valeur boxée
```

Sites qui fuient : `f(2.5)` (paramètre `mixed`), `k.g(2.5)`, `K::h(2.5)`,
`u(2.5)` (paramètre union), `arr.push(2.5)` sur `array<mixed>`,
`mm["x"] = 2.5` (et l'écrasement d'une clé existante),
`var r:int|float = 2.5`, réaffectation `u = 2.5` d'une union,
`var b:bool|null = true`.

Sites qui ne fuient pas : `var v:mixed = 2.5`, `h.x = 2.5`, `return` d'une union.

## Cause

`box_arg_for_mixed_param` (`src/lower/expr.d/helpers.rs`, ~l.135) et la
déclaration/l'affectation d'une union produisent une cellule boxée qui n'est
pas enregistrée comme temporaire possédé (`rc::track`) ni prise par le
stockage (`rc::take`) : personne ne la relâche.

## Correction attendue

Traiter chaque boxing comme une valeur possédée (+1) : temporaire du
statement relâché à sa fin, ou référence transférée au stockage. Profiter de
cette passe pour inventorier tous les sites de boxing (l'audit annoncé par
[memoire-boxing-durcissement](memoire-boxing-durcissement.md) n'a pas été fait).

## Validation

Test `examples/tests/` + un script `--leak-check` sur chacun des sites
ci-dessus (« aucune fuite ») ; mesure mémoire stable en boucle.

## Priorité / Complexité

**Haute** (fuite non bornée, règle du projet) — **Légère**.

## Correctif

- `box_arg_for_mixed_param` (`src/lower/expr.d/helpers.rs`) et l'affectation indexée (`assignments.rs`) enregistrent la cellule boxée comme temporaire du statement (`rc::track`) ; l'appelé et les conteneurs retiennent ce qu'ils gardent.
- Toute union est comptée comme `mixed` (`is_counted`, `src/lower/builder.d/rc_layout.rs`) : elle a la même représentation (IR `Ptr`, scalaires boxés).

Vérifié avec `--leak-check` sur chacun des sites listés (« aucune fuite »). L'inventaire exhaustif des sites de boxing reste à faire (voir [memoire-boxing-durcissement](memoire-boxing-durcissement.md)).
