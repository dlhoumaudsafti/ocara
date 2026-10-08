# Extension JetBrains équivalente à l'extension VS Code

Statut : **à faire** — demandé le 2026-10-08.

## Objectif

Un plugin pour les IDE JetBrains (IntelliJ IDEA, CLion, RustRover,
PhpStorm…) offrant **exactement** les fonctionnalités de l'extension VS Code
(`tools/highlight/vsode/`), sans recoder la sémantique du langage une
troisième fois.

Fonctionnalités à couvrir (inventaire de l'extension VS Code) :

| Fonctionnalité | VS Code aujourd'hui | Source prévue côté JetBrains |
|---|---|---|
| Coloration (dont couleur dédiée des clauses `when`) | grammaire TextMate `ocara.tmLanguage.json` | même grammaire, en bundle TextMate |
| Diagnostics, définition, références, survol documenté, complétion (paramètres nommés), signature help, symboles du document | heuristiques TypeScript, à remplacer par le serveur de langage | `ocara --lsp` |
| CodeLens (références) | `codelens.ts` | LSP (`textDocument/codeLens`) ou inlay équivalent |
| Analyse ocaracs et « Fixer la mise en forme » | `lint.ts`, `fix.ts` | outil externe `ocaracs`, ou serveur de langage |
| Commandes « Compiler le script », « Compiler et lancer », « Afficher le dump » | `compile.ts`, `toolrunner.ts` | actions de menu + configuration d'exécution |
| « Ouvrir la documentation Ocara » | `docs.ts` | action de menu |
| Réglages `compilerPath`, `ocaracsPath`, `lint.enable` | `package.json` | page de réglages du plugin |

## Dépendance

Le plugin ne doit pas reproduire les heuristiques regex de l'extension VS
Code : il s'appuie sur le serveur de langage
([tooling-language-server.md](tooling-language-server.md)), qui doit donc
être fait d'abord. La coloration (TextMate) et les commandes peuvent être
livrées avant.

## Points à trancher

- **Client LSP** : API LSP native de la plateforme IntelliJ, ou plugin
  LSP4IJ (Red Hat) ; vérifier la disponibilité dans les éditions gratuites
  (Community) et les versions d'IDE minimales visées.
- **Emplacement** : `tools/highlight/jetbrains/` à côté de l'extension VS
  Code ; build Gradle (plugin IntelliJ Platform), intégré au Makefile.
- **Parité vérifiée** : liste de contrôle commune aux deux extensions, mise à
  jour à chaque fonctionnalité ajoutée d'un côté (même règle que la
  reconstruction de l'extension VS Code quand l'EBNF change).
- **Publication** : JetBrains Marketplace ou fichier `.zip` installé à la
  main, comme le `.vsix`.

## Priorité / Complexité

**Basse** (outillage) — **Moyenne** une fois le serveur de langage en place
(client LSP, grammaire réutilisée, actions) ; **Structurel** sans lui.
