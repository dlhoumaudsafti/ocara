# Classes homonymes de namespaces différents : une seule est compilée

Statut : **corrigé** (2026-10-09) — constaté via les CodeLens du serveur de
langage sur `mini_project_hexa`.

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

## Correction — identité par namespace

`src/core/analysis.d/homonyms.rs`, avant la fusion des imports :

- un pré-passage parcourt le graphe des imports depuis le fichier d'entrée
  (mêmes règles de résolution que `load_imports`) et relève les types
  déclarés par chaque fichier (classes, generics, interfaces, modules,
  enums) ;
- un nom déclaré par plusieurs fichiers reçoit dans chacun un nom interne
  unique tiré de son namespace (`Format__ns__context_car_app_usecase_helpers`) ;
- chaque fichier voit ses propres déclarations et celles qu'il importe :
  ses références sont réécrites avec `resolve_aliases`, sa déclaration est
  renommée, un import sélectif demande le nom interne ;
- seuls les noms en collision sont touchés ; les messages d'erreur et le
  serveur de langage réaffichent le nom écrit (`homonyms::display`).

Vérifié : deux `Helper` (namespaces `a` et `b`) renvoyant des valeurs
différentes — chaque fichier exécute le sien. Dans `mini_project_hexa`,
chaque contrôleur utilise le `Format` de son contexte (définition et
références comprises). Tests : `src/core/tests_homonyms.rs`.

## Trouvé au passage

- Un appel statique vers une méthode inexistante d'une classe connue
  (`String::to_upper(s)`, `YAML::loadFile(...)`) compilait sans erreur et
  l'appel était abandonné (`null` à l'exécution) : c'est désormais l'erreur
  E01 `undefined symbol 'Classe::m'`. `examples/23_static_method.oc`
  appelait `String::slice`/`String::to_upper`, inexistants (corrigé avec
  `String::capitalize`).
- `make regression` renvoyait 0 même quand `ci/regression.sh` échouait
  (codes de sortie séparés par `;`) : les trois scripts s'exécutent et la
  cible échoue si l'un d'eux échoue.

## Fonctions libres (2026-10-11)

Le même mécanisme couvre les fonctions libres : deux fichiers qui déclarent
chacun `function label()` ont chacun la leur, et chaque fichier appelle celle
qu'il importe (`import label from "util"`), appel ou valeur (`var f = label`).
Les fonctions passent par une clé `fn:` dans les tables, et `resolve_aliases`
réécrit les identifiants qui désignent une fonction homonyme. `main`, le point
d'entrée, n'est jamais renommé. Test : `src/core/tests_homonyms.rs`.

## Limites

- Un fichier qui utilise un type ou une fonction homonyme sans l'importer
  (visibilité globale) voit le premier chargé, comme avant.
- Dans un fichier qui voit une fonction homonyme, une variable locale du
  même nom serait renommée avec elle.

## Reste à faire — audit du 2026-10-13

1. La « limite 1 » ci-dessus est inexacte : un fichier qui voit un homonyme sans l'importer n'obtient pas « le premier chargé » mais un symbole inconnu, et l'appel rend `null` sans diagnostic. Suivi dans [sema-appel-symbole-inconnu-accepte](sema-appel-symbole-inconnu-accepte.md).
2. **Variable locale portant le nom d'une fonction — ignorée.** `var label:Function<string()> = nameless(): string { return "LOCAL" }` puis `return label()` appelle la fonction globale `label` (`fa`) au lieu de la closure (seul un avertissement « variable 'label' is never used ») ; un paramètre `label:string` renvoyé tel quel est refusé (« expected return type 'string', found 'Function<string()>' »). La portée locale doit masquer le nom global, en sema comme au lowering. **Haute (résultat faux) — Légère.**
