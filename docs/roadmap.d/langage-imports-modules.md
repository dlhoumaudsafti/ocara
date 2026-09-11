# Résolution des imports : deux chemins redondants et incohérents

## Constat

La résolution des imports vit intégralement dans `src/main.rs` (~350 lignes), pas dans un module dédié — en décalage avec `docs/workflow-compilation.md`. Pour le format d'import ancien (`import module.Path`), il existe **deux chemins de chargement redondants** :

1. Un chemin récursif correct (src/main.rs:170-340) qui fusionne classes/interfaces/modules/generics et respecte la résolution par namespace.
2. Un second chemin entièrement redondant (src/main.rs:342-393) qui relit les mêmes fichiers mais ne fusionne que classes/functions/consts (**interfaces et modules en sont absents**), sans récursion vers les imports du fichier chargé, et sans résolution par namespace (construction de chemin naïve).

Aujourd'hui masqué car le premier chemin traite déjà le cas avant que le second n'agisse — mais c'est un terrain cohérent avec le bug "interfaces transitives non chargées" documenté par ailleurs (voir [langage-interfaces](langage-interfaces.md)), latent pour des cas plus complexes (fichiers namespacés, classes chargées uniquement via le second chemin).

## Ampleur

Supprimer le second chemin redondant et faire reposer tout le chargement "ancien format" sur le premier (déjà complet) — travail de nettoyage architectural circonscrit à `src/main.rs`, mais qui touche un chemin de code central à toute compilation multi-fichiers.

## Fichiers clés

`src/main.rs` (lignes ~81-450).
