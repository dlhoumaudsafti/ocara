# Serveur de langage (LSP) adossé au compilateur — extension VS Code en client léger

## Constat

L'extension VS Code (`tools/highlight/vsode/`) offre aujourd'hui coloration,
complétion (paramètres nommés), signature help, navigation, CodeLens,
survol documenté, analyse ocaracs, compilation/dump — mais **tout repose sur
des regex en TypeScript**, pas sur le parseur/la sema d'Ocara :

- **Résolution par nom** : deux classes homonymes de contextes différents
  sont confondues ; une méthode d'instance est comptée/proposée pour
  n'importe quel receveur ayant une méthode de ce nom.
- **Types non suivis** : seul un type DÉCLARÉ (`var x:Type`, paramètre,
  propriété) est connu — `var x = f()`, un retour de méthode chaîné, un
  élément de tableau, une variable de boucle restent sans aide.
- **Duplication avec le compilateur** : résolution des imports/namespaces,
  `extends`, constructeur généré des `struct`, blocs runtime partagés,
  sucre d'instance/`Convert`... recodés en TypeScript à côté de leur version
  Rust — chaque évolution du langage est à refaire des deux côtés, avec un
  risque de divergence silencieuse.
- **Dette interne** : deux chemins de résolution d'une méthode coexistent
  (Definition Provider historique de `extension.ts` d'un côté,
  `resolver.ts`/`callsite.ts` de l'autre).
- **Diagnostics de compilation** seulement via la commande « Compiler le
  script » ; analyse ocaracs seulement sur la version enregistrée.

## Proposition

Un serveur de langage qui réutilise directement le pipeline du compilateur
(lexer → parser → chargement des imports → sema), l'extension ne faisant
plus que relayer les requêtes :

- **`ocara --lsp`** (stdio, protocole LSP) : `textDocument/definition`,
  `hover`, `completion`, `signatureHelp`, `references`, `documentSymbol`,
  `publishDiagnostics` (erreurs/avertissements sema EN DIRECT, sur le texte
  en cours d'édition, pas seulement à l'enregistrement).
- Variante plus légère, en première étape : **`ocara --check --json`** —
  diagnostics + table des symboles résolus (déclarations, types inférés,
  cible de chaque appel/référence avec sa position) sérialisés en JSON,
  consommés par l'extension existante à la place de ses heuristiques.

## Ce que ça apporte

- Navigation/références/CodeLens EXACTES (cible réelle résolue par la sema,
  plus de confusion par nom).
- Types inférés disponibles pour la complétion et le survol.
- Erreurs de compilation affichées pendant la frappe.
- Une seule implémentation de la sémantique : l'extension ne recode plus
  rien du langage — suppression de la plupart de `resolver.ts`,
  `callsite.ts`, `runtimecontext.ts`, de la logique de résolution de
  `extension.ts`.

## Choix tranchés (2026-10-08)

- **Protocole** : LSP complet (`ocara --lsp`), livré par étapes — pas de
  format `--check --json` intermédiaire.
- **Dépendances** : crates `lsp-server` + `lsp-types` (celles de
  rust-analyzer, synchrones) et `serde_json`.
- **Extension** : client léger ; chaque fonctionnalité prise en charge par le
  serveur remplace son équivalent TypeScript.
- **ocaracs** : reste un outil séparé, appelé par l'extension.

## Étape 1 — faite (2026-10-08)

- **Analyse partagée** (`src/core/analysis.d/`) : lecture, parsing,
  imports, vérifications de structure, désucrages et sema sortis de
  `main.rs`. Plus aucun `process::exit` sur ce chemin : chaque erreur est un
  `core::diagnostics::Diagnostic`, affiché par le CLI à l'identique,
  publié par le serveur. `compute_aliases` et `expand_runtime_imports`
  retournent leur erreur.
- **Texte non enregistré** : `core::source` lit les fichiers `.oc` (entrée,
  imports, pré-scan des interfaces, runtime imports) avec le texte des
  documents ouverts à la place du disque.
- **Index des références** (`sema::index`) : la sema note, si on le lui
  demande, chaque nom résolu (variable locale avec sa déclaration,
  constante, classe, fonction, méthode, champ, constante de classe,
  argument nommé → paramètre) et son type.
- **Serveur** (`src/lsp/`) : diagnostics en direct à chaque frappe (10 ms
  sur `mini_project_hexa`), erreurs d'un fichier importé rapportées en tête
  du document avec un lien ; survol (type d'une variable, signature et
  commentaires `//` d'une déclaration, documentation des méthodes builtin
  depuis `docs/builtins/*.md` via `builtins-data.json`, méthode héritée
  d'un builtin) ; définition (noms, lignes `import`, `runtime`, `wiring`,
  argument nommé) ; symboles du document. Racine des imports : premier
  dossier contenant un `main.oc` en remontant depuis le fichier.
- **Extension** : client `vscode-languageclient` (`lspclient.ts`) ; le
  fournisseur de définition à regex d'`extension.ts` est supprimé, le survol
  TypeScript ne documente plus que les mots-clés.

## Étapes suivantes

1. **Complétion et signature help** depuis le serveur (types inférés,
   membres de la classe réelle du receveur) — retire `completion.ts`,
   `signature.ts`, puis `resolver.ts`, `callsite.ts`, `runtimecontext.ts`,
   `primitives.ts`.
2. **Références et CodeLens** (`textDocument/references`,
   `textDocument/codeLens`) sur l'index de tous les fichiers du projet —
   retire `codelens.ts`.
3. **Documentation des mots-clés** servie par le serveur (dernier survol
   côté client), pour que l'extension JetBrains l'ait aussi.
4. **Tolérance aux erreurs** : aujourd'hui, une erreur de syntaxe ou
   d'import arrête l'analyse avant la sema (diagnostics seuls, ni survol ni
   définition dans ce document) ; reprise du parseur sur erreur.
5. **Positions** : les spans ne portent qu'un point de départ, le serveur
   retrouve le nom dans la ligne ; plages exactes dans l'AST pour le
   renommage.
6. Limites connues : pas de survol sur une variable jamais utilisée (elle
   est connue par ses utilisations) ; sucre `Convert` (`s.toInt()`) non
   indexé ; colonnes comptées en caractères, pas en unités UTF-16.

## En attendant (gains rapides sur l'extension actuelle)

- Tests automatisés de la partie pure de l'extension (`declarations.ts`,
  découpage des paramètres, ancres de documentation), aujourd'hui inexistants.
- Cache du contexte runtime et des références CodeLens, invalidé par le
  watcher existant (`WorkspaceIndex`) — aujourd'hui tout le workspace est
  relu à chaque survol/complétion dans un fichier runtime.

## Priorité / Complexité

**Moyenne** (outillage, ne bloque pas la stabilité du langage) —
**Structurel** (mode serveur du compilateur, sema tolérante aux erreurs,
spans en plages ; réécriture de l'extension en client léger).

## Fichiers clés

`src/main.rs` (pipeline, `process::exit` à remplacer par des erreurs
remontées), `src/core/cli.rs`, `src/sema/`, `src/parsing/token.rs` (spans),
`tools/highlight/vsode/src/` (extension), `Makefile`.
