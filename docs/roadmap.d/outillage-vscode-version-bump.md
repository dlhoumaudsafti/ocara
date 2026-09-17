# Extension VSCode restée en 0.1.0 après le passage du langage en 1.0.0

## ✅ Terminé

Version alignée sur celle du langage : `tools/highlight/vsode/package.json` passe à `1.0.0`, `.vsix` régénéré (`ocara-language-1.0.0.vsix`), et toutes les occurrences de la commande d'installation dans `tools/highlight/vsode/README.md` mises à jour (installation, mise à jour, désinstallation/réinstallation) — commit `a514b07`.

## Constat

Le bump de version du 2026-09-16 (`feat: Version bumpée à 1.0.0 dans les 6 Cargo.toml...`) a délibérément laissé l'extension VSCode de côté — le message de commit le dit explicitement : *« L'extension VSCode (`tools/highlight/vsode/`) volontairement non touchée — versionnée indépendamment par convention du projet »*. `tools/highlight/vsode/package.json:5` affiche toujours `"version": "0.1.0"`, et le `.vsix` pré-compilé livré dans le dépôt est toujours `ocara-language-0.1.0.vsix`.

Ce n'est pas un problème de couverture fonctionnelle : une vérification rapide du grammar file (`tools/highlight/vsode/syntaxes/ocara.tmLanguage.json`) montre que les mots-clés récents (`parent`, `generic`, `namespace`, `modules`, `variadic`, `async`, `resolve`, `consumed`, `scoped`, `nameless`...) y sont déjà tous présents, et le `README.md` de l'extension documente déjà l'autocomplétion `parent.`. L'extension suit donc le langage en pratique (dernier commit la touchant : 2026-09-15, veille du bump 1.0.0) — c'est uniquement son **numéro de version** qui n'a jamais été mis à jour depuis sa création, alors que le langage qu'elle sert vient de passer en 1.0.0.

## Ce qui est demandé

Suivre la méthode de travail habituelle du projet pour un changement touchant l'extension (`docs/roadmap.md`, § Méthode de travail : lire le `README.md` de l'extension, désinstaller la version en cours, recompiler, réinstaller) et :
1. Décider d'un schéma de version pour l'extension, indépendant de celui du langage (déjà le principe actuel) — par exemple aligner sur `1.0.0` pour marquer sa propre maturité, ou repartir sur un `0.x` propre à son cycle de release.
2. Régénérer le `.vsix` avec le nouveau numéro et le nom de fichier correspondant.
3. Mettre à jour `tools/highlight/vsode/README.md` (commande `code --install-extension ocara-language-X.Y.Z.vsix` actuellement câblée en dur sur `0.1.0`).

## Priorité / Complexité

**Terminé.** Était Priorité Basse, Complexité Simple — confirmé : bump de version + recompilation + republication du `.vsix`, pas de changement de code.

## Fichiers clés

`tools/highlight/vsode/package.json`, `tools/highlight/vsode/README.md`, `tools/highlight/vsode/*.vsix`.
