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

## Points à trancher

- **Protocole** : LSP complet dès le départ, ou `--check --json` d'abord
  (plus simple, appel ponctuel par fichier) ?
- **Analyse incrémentale / tolérance aux erreurs** : la sema s'arrête
  aujourd'hui à la première erreur bloquante de certaines phases (imports,
  `process::exit` dans `src/main.rs`) — un serveur de langage doit
  continuer sur un fichier partiellement invalide (texte en cours de
  frappe). Demande de remonter des erreurs au lieu de quitter le processus.
- **Positions** : les spans des nœuds AST ne couvrent souvent qu'un point de
  départ (ligne/colonne), pas une plage — à étendre pour des surlignages et
  des renommages précis.
- **Dépendances** : implémentation LSP maison (JSON-RPC sur stdio) ou crate
  dédiée (`tower-lsp`, `lsp-server`) — impact sur le temps de build et la
  règle « jamais `cargo` direct, toujours via le Makefile ».
- **ocaracs** : intégré au serveur (diagnostics de style sur le texte en
  cours) ou laissé en outil séparé.

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
