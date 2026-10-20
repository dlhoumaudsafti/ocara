# Compilateur : complexité résiduelle sur des fonctions géantes

Constaté le 2026-10-20 en clôturant [compilateur-limites-echelle](compilateur-limites-echelle.md).
Aucun plantage : temps de compilation encore super-linéaire au-delà des
tailles réalistes.

## Constat

1. **`elseif` × 40 000 : 9,9 s** (10 000 : 1,1 s). Échantillonné : calcul de
   l'arbre des dominateurs de Cranelift 0.105 (`compute_idom` →
   `common_dominator`) — le bloc de fusion final a n prédécesseurs, chacun à
   profondeur ~n. Piste : fusions chaînées (chaque bras rejoint la fusion du
   `elseif` suivant) pour que chaque bloc n'ait que deux prédécesseurs ;
   vérifier que Cranelift replie les sauts vides (aucun coût à l'exécution).
2. **50 000 `if` imbriqués : 130 s** (5 000 : 0,9 s). Origine non isolée
   (copie de `owned_locals` par branche dans `lower_if`, profondeur de
   `block_scope_stack`, ou arbre des dominateurs).
3. L'imbrication des **instructions** n'est pas bornée par un diagnostic
   comme celle des expressions (`MAX_EXPR_DEPTH`) : la pile de 1 Gio du
   thread de compilation suffit jusqu'à 50 000 niveaux, au-delà non mesuré.

## Priorité / Complexité

**Haute** (temps de compilation non borné) — **Légère** à **Structurel**.

## Fichiers clés

`src/lower/stmt.d/statements.d/control_flow.rs` (`lower_if`,
`lower_elseif_chain`), `src/parsing/parser.d/statements.rs`.
