# Méthode sucrée sur un récepteur indexé : repli sur `String`, appel perdu

Statut : **corrigé** le 2026-10-13 (constaté le même jour par l'audit de la roadmap).

## Constat

```ocara
var nest:array<array<int>> = [[1, 2, 3]]
nest[0].push(5)          // émet String_push : l'élément n'est jamais ajouté (len reste 3)
nest[0].len()            // émet String_len : rend 1
var m:map<string, array<int>> = {"a": [1, 2, 3]}
m["a"].len()             // 1 au lieu de 3
var mm:map<string, map<string,int>> = {"x": {"y": 1}}
mm["x"].has("y")         // null
for k has v in m { v.len() }   // 1 au lieu de 3
```

Les formes statiques (`Array::push(nest[0], 5)`, `Array::len(v)`) sont
correctes. [memoire-array-push-large-int-boxed](memoire-array-push-large-int-boxed.md)
annonçait à tort que la forme sucrée gérait un élément indexé.

## Cause

- `resolve_receiver_class` ne traite pas `Expr::Index` (ni la valeur d'une
  boucle sur map) ; le lowering devine alors `"String"`
  (`src/lower/expr.d/lower.rs`, ~l.508-517) ;
- un appel vers un symbole inexistant (`String_push`, `String_has`) est ignoré
  en silence par le codegen (`src/codegen/emit.d/instructions.d/calls.rs`,
  ~l.62-69, « Autre fonction inconnue : ignorée »).

## Correction attendue

1. Résoudre la classe d'un récepteur indexé à partir du type d'élément connu
   (`elem_ast_types`, type sema) ; même chose pour la valeur `v` d'un
   `for k has v`.
2. Remplacer le « ignoré » du codegen par une erreur interne (un appel inconnu
   ne doit jamais disparaître) — voir aussi
   [sema-appel-symbole-inconnu-accepte](sema-appel-symbole-inconnu-accepte.md).

## Validation

Test `examples/tests/` sur `push`/`len`/`has`/`get` via `a[i]`, `m[k]` et la
valeur d'une boucle sur map ; `make regression`.

## Priorité / Complexité

**Haute** (résultat faux silencieux sur un usage de base) — **Légère**.

## Correctif

`resolve_receiver_class` résout un récepteur indexé par son type d'élément (`receiver_class_of_type` : `String`/`Array`/`Map`/classe/générique) ; la valeur `v` de `for k has v` reçoit ses métadonnées (`register_const_meta`). Test : `examples/tests/88_…` (`indexedReceiverTest`).
