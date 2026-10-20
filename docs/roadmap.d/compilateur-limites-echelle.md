# Compilateur et binaires : débordements de pile et complexité super-linéaire

**Clos le 2026-10-20** (reliquats sur des tailles extrêmes : [compilateur-limites-echelle-residuelles](compilateur-limites-echelle-residuelles.md)). Vérifié :
1. Compilation dans un thread à pile de 1 Gio (`src/main.rs`) + borne de profondeur d'expression à 50 000 avec diagnostic (`MAX_EXPR_DEPTH`, `expressions.rs`) : 20 000 parenthèses et 100 000 termes compilent, au-delà erreur propre au lieu d'un abort.
2. Débordement de pile d'un programme : message « débordement de pile » via pile alternative et gestionnaire SIGSEGV/SIGBUS (`runtime/src/stack_guard.rs`, appelé en tête du `main` généré et de chaque thread du runtime), code de sortie inchangé (139). Hors Android et Windows.
3. `select_variants` en une passe : 50 000 instructions, 7,5 s → 0,3 s à `--check`.
4. Tables globales partagées (`Rc`) au lieu d'être recopiées par fonction, et plus de copie des tables du codegen par fonction (`emit_function`) : 4 000 fonctions 11,1 s → 0,4 s ; 2 000 classes 3,7 s → 0,4 s. Type IR des `Binary` mémoïsé par instruction (`binary_ir_types`) : somme de 20 000 termes 14 s → 0,7 s.
5. `elseif` × 10 000 : 5,6 s / 1,6 Go → 1,1 s / 250 Mo. Origine des 1,6 Go : une `Variable` Cranelift par valeur HIR (table par variable indexée par bloc) — `value_slots.rs` ne garde des `Variable` que pour les valeurs vivantes entre blocs. Vérificateur Cranelift appelé par fonction, sauf au-delà de 2 000 blocs (test de dominance quadratique).


Audit du 2026-10-19. Mesuré sur une machine chargée : les pentes sont nettes,
les valeurs absolues bruitées.

## Débordements de pile

1. **Le compilateur déborde sur une expression profonde** (abort, rc 134,
   « thread 'main' has overflowed its stack ») : parenthèses imbriquées, 600
   niveaux passent et 800 plantent ; somme plate `a + a + … + a`, 3 000 termes
   passent et 5 000 plantent. Parseur descendant (un frame par niveau de
   précédence, `src/parsing/parser.d/expressions.rs`), puis passes récursives
   sur l'arbre penché à gauche. Piste : diagnostic de profondeur, thread de
   compilation avec une grande pile, parseur à précédence itératif.
2. **Récursion profonde dans un programme : SIGSEGV muet** (`depth(1000000)`
   avec `return 1 + depth(n - 1)` ; ~100 000 niveaux avec une chaîne locale).
   Piste : page de garde + gestionnaire `SIGSEGV` sur pile alternative
   (`sigaltstack`) qui affiche « stack overflow ».

## Complexité super-linéaire du compilateur

3. **Parseur O(n²) par bloc** : `select_variants`
   (`src/parsing/parser.d/when.rs` ~l.231-245) fait `seen.contains` plus un
   balayage pour chaque élément. Une fonction de 10 000 instructions : 0,3 s
   à `--check` ; 50 000 : 8,6 à 13 s.
4. **Abaissement O(F²) en nombre de fonctions** : `lower_func` clone toutes
   les tables globales par fonction (`src/lower/builder.d/functions.rs`
   ~l.130-145). 1 000 fonctions : 1,45 s ; 5 000 : 46,9 s ; 2 000 classes de
   2 méthodes : 15,5 s.
5. **Longue chaîne `elseif` : temps et mémoire quadratiques dans Cranelift**
   (5 000 : 1,3 s et 424 Mo ; 10 000 : 5,0 s et 1,62 Go). Le vérificateur
   Cranelift reste actif (`enable_verifier=true`,
   `src/codegen/emit.d/emitter.rs` ~l.65-79). Origine des 1,6 Go à isoler.

## Priorité / Complexité

**Haute** (crash du compilateur, crash muet des programmes) — **Légère** à
**Structurel**.

## Fichiers clés

`src/parsing/parser.d/expressions.rs`, `src/parsing/parser.d/when.rs`,
`src/lower/builder.d/functions.rs`, `src/codegen/emit.d/emitter.rs`,
`src/main.rs` (pile du thread principal), `runtime/src/` (gestionnaire de signal).
