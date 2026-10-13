# `for i in a until b` alloue un tableau de `b − a` entiers

Statut : **à faire** — mesuré le 2026-10-13 (audit de la roadmap).

## Constat

`__range` (`runtime/src/lib.rs`, ~l.626) construit le tableau complet avant la
boucle : 1 M itérations → 10 Mo, 3 M → 25,8 Mo (contre 3,9 Mo pour un `while`
équivalent). Pas une fuite (libéré en fin de boucle), mais une mémoire en
O(n) pour une simple boucle de comptage.

`docs/benchmarking.md` (~l.192) le signale comme « piège », avec l'ancienne
syntaxe `0..N` (retirée).

## Correction attendue

Dans `lower_for_in`, lorsque l'itérable est une plage `to`/`until`, générer
une boucle compteur sans tableau ; garder `__range` pour une plage utilisée
comme valeur.

## Validation

Mesure mémoire constante sur 3 M itérations ; `07_loopsTest`,
`15_operatorsTest` ; mise à jour de `benchmarking.md`.

## Priorité / Complexité

**Moyenne** (performance, non bloquant) — **Légère**.
