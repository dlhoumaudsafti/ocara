---
name: Copilot Agent Instructions
description: An agent that provides instructions for using the Copilot agent to contribute to the Ocara project.
argument-hint: Use the instructions provided to contribute to the Ocara project effectively.
target: github-copilot
model: Claude Sonnet 4.5

---

# Copilot Agent Instructions

## Contexte
Ocara est un langage de programmation en cours de développement. Il est conçu pour être simple, expressif et efficace. Le projet est encore en phase de développement actif, avec de nombreuses fonctionnalités à venir. Il est écris en Rust.
La source du projet ce trouve dans le dossier `src/` et `runtime/`. Les exemples d'utilisation du langage se trouvent dans le dossier `examples/`. La documentations se trouve dans le dossier `docs/`.

## Règles
- ne jamais utiliser la commande `cargo`. On préfère les commande avec le makefile (ex: `make build`, `make test`, etc.) pour éviter les problèmes de configuration et de dépendances.
- En cas de doute sur la syntaxe du langage, se référer à la documentation dans le dossier `docs/` et princiaplement sur la docummentation [docs/EBNF.md](EBNF.md) et aux `docs/builtins/*.md`.
- Pour comprendre comment le copilateur Ocara fonctionne. Se référer à la documentation [docs/workflow-compilation.md](workflow-compilation.md).
- Pour commprendre comment les builtins sont implémentés, tu peux utiliser la documentation [docs/adding-builtins.md](adding-builtins.md).
- Ne pas suggérer de code qui a été supprimé dans les récents commits. Se référer à la section "Recently edited files" ci-dessous pour voir les fichiers récemment modifiés et éviter de suggérer du code qui a été supprimé.
- Toujours respecter le système de découpe dans la source pour eviter d'avoir des fichiers de code trop long.
- Toujours faire en sorte d'avoir le code le plus optimisé en terme de puissance de calcul, en utilisant les bonnes structures de données et algorithmes pour chaque cas d'utilisation.
- pas utiliser de 2>&1 et de tail dans les commandes shell. L'utilisateur doit pouvoir voir la sortie du terminal en temps réel pour comprendre ce qui se passe et détecter les erreurs. Si une commande génère beaucoup de sortie, lire le fichier content.txt généré par le copilateur.

## Style
- Code lisible, typé strictement
- Pas de commentaires inutiles
- Noms explicites
- Pas de code dupliqué
- Toujours faire en sorte d'avoir le code le plus simple possible, en évitant les optimisations prématurées et en privilégiant la clarté du code.