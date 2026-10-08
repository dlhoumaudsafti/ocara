# Grammaire Tree-sitter pour Ocara

Statut : **à faire** — demandé le 2026-10-08.

## Objectif

Ajouter au projet une grammaire Tree-sitter d'Ocara, pour colorer le code
Ocara partout où il apparaît : fichiers `.oc`, blocs ```` ```ocara ```` des
Markdown (README, docs), sur GitHub, GitLab, VS Code et les éditeurs.

```
un_projet_ocara/
├── README.md            exemples de code ```ocara
├── src/
├── tree-sitter-ocara/
│   ├── grammar.js
│   ├── src/             parser.c généré (tree-sitter generate)
│   ├── queries/
│   │   └── highlights.scm
│   └── package.json
├── .gitattributes       *.oc linguist-language=Ocara
└── ...
```

## Ce que Tree-sitter couvre, et ce qu'il ne couvre pas

Constat à garder en tête avant d'engager le travail : chaque plateforme
colore avec son propre moteur.

| Cible | Moteur de coloration | Effet d'une grammaire Tree-sitter dans le dépôt |
|---|---|---|
| Neovim, Helix, Zed, Emacs (29+) | Tree-sitter | Coloration directe (`highlights.scm`) |
| GitHub (fichiers, Markdown) | Linguist : grammaires TextMate | Aucun tant qu'Ocara n'est pas un langage reconnu par Linguist |
| GitLab | Rouge (lexers Ruby) | Aucun : il faut un lexer Rouge |
| VS Code | TextMate (extension `tools/highlight/vsode`) | Aucun : l'extension existante reste la voie |

Conséquences :

- **GitHub** : le langage doit être ajouté à
  [github-linguist/linguist](https://github.com/github-linguist/linguist),
  qui exige un usage public suffisant (quelques centaines de dépôts
  utilisant l'extension). La grammaire TextMate de l'extension VS Code
  (`ocara.tmLanguage.json`) peut servir de grammaire Linguist. En attendant,
  `.gitattributes` ne peut associer `*.oc` qu'à un langage déjà connu
  (coloration approximative, par ex. `linguist-language=Kotlin`).
- **GitLab** : écrire un lexer Rouge et le proposer en amont, ou accepter
  l'absence de coloration.
- **Tree-sitter** reste utile en soi : éditeurs modernes, et base commune
  pour d'autres outils (navigation, serveur de langage, voir
  [tooling-language-server.md](tooling-language-server.md)).

## Travail

- `grammar.js` dérivée de [docs/EBNF.md](../EBNF.md) : déclarations, types
  génériques (`array<T>`, `map<K,V>`, `message<T>`), `var`/`scoped`/
  `consumed`/`const`, gabarits `${…}`, comparaisons en toutes lettres,
  `nameless`, `emit`, `try`/`on`, blocs runtime.
- `queries/highlights.scm` alignée sur les couleurs de l'extension VS Code
  (même distinction des mots-clés, et le moment venu la couleur dédiée
  aux conditions `when`, voir
  [reflexion-declarations-conditionnelles-when.md](reflexion-declarations-conditionnelles-when.md)).
- Tests du corpus Tree-sitter (`test/corpus/`) à partir des
  `examples/*.oc` ; vérifier que tout `examples/` se parse sans nœud
  `ERROR`.
- Commande `make` pour régénérer le parser ; garder la grammaire synchronisée
  avec EBNF.md (même règle que l'extension VS Code).
- Emplacement : `tree-sitter-ocara/` à la racine comme demandé, ou
  `tools/highlight/tree-sitter-ocara/` à côté de l'extension VS Code — à
  trancher.
- Publication éventuelle (npm `tree-sitter-ocara`, crate Rust) pour que les
  éditeurs puissent l'installer.
