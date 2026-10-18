# Fermeture capturant une variable de boucle au nom réutilisé — corrigé

## Constat (2026-10-18)

Une fermeture créée dans une boucle capture la variable de boucle. Si ce nom
avait déjà servi à une boucle précédente de la même fonction, les valeurs
capturées étaient fausses (`222` au lieu de `012` avec une plage, une valeur
arbitraire avec un tableau littéral), puis le programme se bloquait.

```ocara
for i in 0 until 3 { s = s + i.toStr() }
var fns:array<Function<int()>> = []
for i in 0 until 3 { fns.push(nameless(): int { return i }) }
for f in fns { s = s + f().toStr() }   // attendu 012, obtenu 222, puis blocage
```

## Cause

La variable d'une boucle est déclarée hors du bloc du corps : elle reste
dans `locals` après la boucle. La pré-promotion de la boucle suivante
(`hoist_closure_promotions_before_loop`) promouvait cet homonyme au tas, et
`heap_promoted` (indexé par nom) restait marqué. La nouvelle déclaration
créait un slot de pile, mais chaque accès passait par
`__locked_cell_get`/`set` sur ce slot (mémoire corrompue).

## Correctif

- `declare_local` retire le nom de `heap_promoted` : une nouvelle
  déclaration est toujours un slot neuf. `lower_block` rétablit la promotion
  d'un nom externe masqué par une déclaration du bloc.
- La pré-promotion ignore les variables que la boucle déclare
  (`loop_vars`).
- Générateur : une nouvelle déclaration d'une variable capturée rend la
  cellule du frame (`reset_gen_cell`). Chaque itération a sa cellule ;
  toutes les fermetures d'une boucle lisaient sinon la dernière valeur
  (`20,20,20`), même sans nom réutilisé.

## Trouvés en passant, corrigés

- **Tableau littéral itéré directement** (`for j in [0, 1, 2]`) : construit
  en `mixed`, alors que la sema type `j` en `int`. `(j * 10).toStr()`
  affichait une adresse pour `0`. Un littéral à constantes `int`/`float`/
  `bool` d'un même type est maintenant construit brut (`literal_elem_type`).
- **Négation d'un flottant** : `Inst::Neg` était toujours entier.
  `var g:float = -2.5` valait `-1.75` (négation entière des bits). Le
  lowering passe le type de l'opérande, et le codegen émet `fneg`.
- **`%` sur des flottants** : `srem` sur les bits donnait une valeur
  absurde. Calcul `x − trunc(x / y) × y` (signe du dividende, comme
  `fmod`).
- **`for k has v in m`** : la sema déclarait `k` et `v` en `mixed`
  (`v.toStr()` refusé) alors que le lowering les lit en `K`/`V`. Elles
  sont désormais typées `K`/`V` quand le type de la map est connu.

## Tests

`examples/tests/93_closure_loop_var_reused_nameTest.oc`,
`examples/tests/15_operatorsTest.oc` (`floatNegationAndModuloTest`).
