# Chaque lecture de `System::OS` alloue une chaîne jamais libérée

Statut : **à corriger** — constaté le 2026-10-13 (audit de la roadmap).

## Constat

```ocara
for i in 0 until 2000 {
    var s:string = System::OS
}
// --leak-check : 2000 × string non libérées
```

Constaté en vérifiant les initialiseurs de `property` ; la fuite est
indépendante de l'initialiseur.

## Cause (à confirmer)

`__system_os` (et vraisemblablement `__system_arch`, déclarés dans
`src/codegen/desc.d/lowlevel.rs`) rend une chaîne allouée, mais le lowering de
la constante `System::OS` ne l'enregistre pas comme temporaire possédé — ou le
runtime alloue une nouvelle chaîne à chaque lecture au lieu d'une chaîne
statique. Vérifier aussi `System::ARCH` et `System::BUILD`.

## Correction attendue

Rendre une chaîne statique (immortelle) ou traiter le résultat comme un
temporaire possédé.

## Validation

`--leak-check` sans fuite sur une boucle de lectures de `System::OS`,
`System::ARCH`, `System::BUILD` ; test `examples/tests/`.

## Priorité / Complexité

**Haute** (fuite, règle du projet) — **Simple**.
