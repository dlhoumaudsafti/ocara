# Résolution des imports : deux chemins redondants et incohérents

## Constat

La résolution des imports vit intégralement dans `src/main.rs` (~350 lignes), pas dans un module dédié — en décalage avec `docs/workflow-compilation.md`. Pour le format d'import ancien (`import module.Path`), il existe **deux chemins de chargement redondants** :

1. Un chemin récursif correct (src/main.rs:170-340) qui fusionne classes/interfaces/modules/generics et respecte la résolution par namespace.
2. Un second chemin entièrement redondant (src/main.rs:342-393) qui relit les mêmes fichiers mais ne fusionne que classes/functions/consts (**interfaces et modules en sont absents**), sans récursion vers les imports du fichier chargé, et sans résolution par namespace (construction de chemin naïve).

Aujourd'hui masqué car le premier chemin traite déjà le cas avant que le second n'agisse — mais c'est un terrain cohérent avec le bug "interfaces transitives non chargées" documenté par ailleurs (voir [langage-interfaces](langage-interfaces.md)), latent pour des cas plus complexes (fichiers namespacés, classes chargées uniquement via le second chemin).

**Confirmé concrètement** : `examples/project/tests/mainTest.oc` (`import main`, ancien format à un seul segment) échoue à la compilation avec `interface 'Printable' not found`. Ce cas précis résout au symbole **fonction** `main` du fichier via le premier chemin (recherche par nom, `main` étant bien enregistré comme une fonction — voir `src/parsing/parser.d/tests.rs:49`), puis c'est le **second chemin redondant** qui fusionne réellement les classes `Score`/`Student` du fichier (fusion inconditionnelle de tout `mod_prog.classes`, sans regarder ce qui a été demandé) — sans jamais fusionner `Printable`/`Comparable`, qui n'existent que dans `mod_prog.interfaces`. Voir [qualite-couverture-tests](qualite-couverture-tests.md) pour le détail de cette investigation.

**Ne pas confondre avec un bug voisin, déjà corrigé séparément** : la déduplication des imports (`processed_files`/`processed_imports`) qui ignorait un second symbole demandé du même fichier (`import Circle from "X"` puis `import Rectangle from "X"`) a été corrigée — voir [langage-interfaces](langage-interfaces.md) et [qualite-couverture-tests](qualite-couverture-tests.md). Ce fichier-ci documente uniquement le problème du second chemin de chargement (section 4b), qui reste ouvert.

## Ampleur

Supprimer le second chemin redondant et faire reposer tout le chargement "ancien format" sur le premier (déjà complet) — travail de nettoyage architectural circonscrit à `src/main.rs`, mais qui touche un chemin de code central à toute compilation multi-fichiers.

## Fichiers clés

`src/main.rs` (lignes ~81-450).
