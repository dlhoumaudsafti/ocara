# `Map::forEach` sur une map concrète `int`/`float` — SIGSEGV

Statut : **corrigé** le 2026-10-13 (constaté le même jour par l'audit de la roadmap).

## Constat

```ocara
var m:map<string,int> = {"a": 3, "c": 100000}
var total:int = 0
Map::forEach(m, nameless(key:mixed, value:mixed): void { total = total + 1 })
// exit 139 (SIGSEGV), quel que soit le corps de la closure
```

- `{"a": 3, "b": 4}` (petits entiers) : OK ;
- valeur ≥ 65536 ou `map<string,float>` (`{"c": 2.5}`) : SIGSEGV ;
- `map<string,mixed>` et `map<string,string>` : OK.

## Cause

`Map_forEach` (`runtime/src/lib.rs`, ~l.1314-1330) transmet la valeur brute
telle quelle au paramètre `value:mixed` de la closure : un grand entier ou les
bits d'un flottant y sont lus comme un pointeur.

`docs/builtins/Map.md` (~l.170) ne décrit qu'un « résultat incorrect » en
arithmétique et renvoie vers une fiche supprimée. Aucun exemple ni test
n'utilise `Map::forEach`.

## Correction attendue

Boxer chaque valeur (et clé) selon la forme de la map avant l'appel — même
information que pour [JSON/YAML](stdlib-json-yaml-valeurs-brutes.md) — ou
faire générer la boucle par le compilateur, qui connaît `V`.

## Validation

Test `examples/tests/` sur `map<string,int>` (petites et grandes valeurs),
`map<string,float>`, `map<string,bool>`, `map<string,mixed>` ; mise à jour de
`Map.md`.

## Priorité / Complexité

**Haute** (crash) — **Légère**.

## Correctif

Le compilateur passe à `Map_forEach` la forme des valeurs de la map en argument caché (`push_hidden_leaf_shape`, `src/lower/expr.d/helpers.rs`, même mécanisme que `JSON::encode`) ; le runtime boxe chaque valeur brute `int`/`float`/`bool` avant l'appel de la closure et la relâche après. Test : `examples/tests/88_…` (`forEachConcreteMapTest`), `--leak-check` sans fuite.
