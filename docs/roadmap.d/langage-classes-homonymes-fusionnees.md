# Classes homonymes de namespaces différents : une seule est compilée

Statut : **à faire** — constaté le 2026-10-09 (CodeLens du serveur de
langage sur `mini_project_hexa`).

## Constat

Les symboles d'un programme sont identifiés par leur **nom simple**. Quand
deux fichiers de namespaces différents déclarent chacun une classe du même
nom, la fusion des imports n'en garde qu'une (`dedup` dans
`src/core/analysis.d/imports.rs`, première chargée), et la table des
symboles est indexée par nom : tout le programme utilise cette classe.

`examples/advanced/mini_project_hexa` déclare trois `Format` et deux
`Flash` :

```
context/home/app/usecase/helpers/Format.oc     namespace context.home…
context/car/app/usecase/helpers/Format.oc      namespace context.car…
context/search/app/usecase/helpers/Format.oc   namespace context.search…
```

Les versions diffèrent (`rem.toStr()` d'un côté, `Convert::intToStr(rem)`
de l'autre) ; le contrôleur du contexte `car`, qui importe
`context.car.app.usecase.helpers.Format`, exécute en réalité le `Format` du
contexte `home`. Ici le résultat est le même, mais rien ne le garantit :
une différence de comportement passerait inaperçue, sans diagnostic.

## Pistes

1. **Court terme** : erreur de compilation quand deux déclarations
   *différentes* portent le même nom simple dans un même programme (en
   indiquant les deux fichiers), au lieu d'en ignorer une silencieusement.
2. **Structurel** : identité qualifiée par namespace
   (`context.car…Format`), résolution de chaque usage selon les imports
   du fichier qui l'écrit, mangling des symboles avec le namespace.

## Priorité / Complexité

**Haute** (comportement silencieusement différent de ce qu'on lit) —
**Légère** pour la piste 1, **Structurelle** pour la piste 2.
