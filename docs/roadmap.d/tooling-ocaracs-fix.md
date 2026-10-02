# ocaracs `--fix` — implémenté

Documentation utilisateur : `tools/ocaracs/README.md`, section « Correction
automatique — `--fix` ».

## Ce qui a été tranché

- **Règles corrigées** : R01 (indentation), R02, R03, R04, R06, R10, R11 et
  le nommage (R07/R08/R09/R12/R13). R05 (ligne trop longue) n'est pas
  corrigeable automatiquement. Le contenu des backticks multilignes n'est
  jamais modifié.
- **Indentation** : niveau recalculé d'après les `{ }`, `( )`, `[ ]`.
  Plusieurs ouvrants sur une même ligne (`route(…, nameless(…) {`) ne
  comptent qu'un niveau, les fermants en tête de ligne sont retirés avant de
  mesurer, et une ligne de continuation prend un niveau de plus.
- **Renommage sur tout le projet** : déclaration et usages dans tous les
  `.oc` sous la racine (dossier du `.ocaracs` le plus proche, sinon le dossier
  analysé). Les membres sont aussi renommés après `.`/`::`, les autres noms
  non. Dans les imports, seul le dernier segment est renommé, et le fichier
  `Ancien.oc` d'une classe renommée devient `Nouveau.oc`. Le renommage est
  ignoré et signalé dans cinq cas : collision avec un nom existant, mot
  réservé, styles contradictoires, nom référencé dans un fichier non `.oc`
  (templates HTML), et suffixe `Test` d'ocaraunit perdu.

## Vérification sur `examples/` (copie)

- 1574 avertissements avant, 116 après : lignes trop longues et renommages
  ignorés.
- 316 identifiants renommés (1122 occurrences) dans 202 fichiers.
- Les 91 fichiers qui compilaient compilent toujours.
- Tous les tests ocaraunit passent toujours (957 + 50).
- La sortie à l'exécution des 59 exemples non interactifs est identique, aux
  valeurs non déterministes près (hasard, PID, horodatages).

Le corpus du dépôt lui-même n'a pas été corrigé.

## Fichiers clés

`tools/ocaracs/src/main.rs` (CLI, parcours, `fix_all`), `fix.rs`
(mise en forme), `rename.rs` (plan et application du renommage), `decls.rs`
(déclarations, partagé avec l'analyse), `check.rs` (analyse), `text.rs`
(chaînes/backticks/commentaires). Tests : `make tests-tools`.
