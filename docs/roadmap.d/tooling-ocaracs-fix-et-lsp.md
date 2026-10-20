# ocaracs `--fix`, serveur de langage et extension VS Code

Audit du 2026-10-19 ; corrigé le 2026-10-20, sauf « Reste ».

## Corrigé

- **ocaracs `--fix`** : fins de ligne conservées (CRLF, chaînes backtick
  intactes) ; collisions de renommage détectées ; membres renommés seulement
  sur un receveur du projet (`members.rs`) ; champ de struct, fonction et
  fichier, import suivi d'un commentaire cohérents (`rewrite.rs`) ; `when`
  réservé ; `'{'` dans une chaîne ignoré ; indentation (`*` de SQL, `import
  ocara.*`, `when default function`) ; analyse d'un projet depuis `main.oc`
  résolue comme le compilateur (`project.rs`) ; non-UTF-8, option inconnue et
  erreurs de `.ocaracs` signalés ; colonnes en caractères. Tests dans
  `tools/ocaracs` (35).
- **Serveur de langage** : un fichier importé modifié ou fermé réanalyse les
  documents qui l'importent (`dependents`, `src/lsp/mod.rs`) ; soulignement
  sur le nom du membre (`word_at` saute `.`/`::`) et message « method … not
  found » (`SemaError::MethodNotFound`) ; renommage refusé en cas de conflit
  (nom déjà déclaré, classe builtin — `src/lsp/rename_check.rs`, tests
  `tests_rename_check.rs`).
- **Extension** : le délai du benchmark tue le programme mesuré (groupe de
  processus) ; bilan et profil conservés sur sortie non nulle ; historique
  `leak`/`growth` séparé ; résultat attaché au script mesuré ; fichiers de
  croissance dans le dossier temporaire ; lint sans diagnostics périmés ni
  double exécution ; redémarrage du serveur (commande et changement de
  `ocara.compilerPath`) ; grammaire (`\``, `\${`, `generic`, `if (`, `import`
  commenté/indenté, HTML des gabarits borné au backtick) ; ancres des liens
  de documentation ; diagnostics de compilation effacés ; « Compiler et
  lancer » sans shell ; `'` non fermé dans les commentaires.
- **CI / Makefile** : `ci/start.sh` sort en erreur si une étape échoue ou si
  rien n'est traité ; `ci/memory_bound.sh` sous `timeout` ; `lint-examples`
  ne tolère que le code 1 (avertissements de style) ; `tests-tools` lance
  aussi les tests d'ocaraunit.

## Reste

- Serveur de langage mono-thread, sans `$/cancelRequest` : la première
  construction de l'index (1 à 1,8 s sur `mini_project_hexa`) bloque les
  autres requêtes.
- Documents `untitled:` sans diagnostics ni survol.
- Renommage de fichier envoyé (`documentChanges`) sans vérifier les
  capacités du client (à prendre en compte pour le client JetBrains).

## Priorité / Complexité

**Basse** (reste) — **Légère** à **Structurel** (requêtes concurrentes).

## Fichiers clés

`tools/ocaracs/src/`, `src/lsp/`, `tools/highlight/vsode/src/`,
`tools/highlight/vsode/syntaxes/`, `ci/`, `Makefile`.
