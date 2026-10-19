# CLI du compilateur : source écrasé, fichiers temporaires, options ignorées

Audit du 2026-10-19. Reproduit sur le binaire courant.

## Constat

1. **`ocara x.oc -o x.oc` écrase le source par l'exécutable**, exit 0, sans
   avertissement (`src/core/cli.rs` ~l.134, `src/main.rs` ~l.129).
   L'objet temporaire est `output.with_extension("o")` : `-o build/app.1.2`
   écrit `build/app.1.o` (peut écraser un fichier), et ce `.o` reste à côté de
   chaque binaire.
2. **Runtime extrait dans `/tmp` à chaque compilation (79 Mo), laissé en
   place si la compilation est interrompue** : `/tmp/libocara_runtime_<pid>.a`
   (11 fichiers orphelins trouvés), nettoyage seulement en fin normale
   (`src/codegen/link.rs` ~l.45-50, ~l.203). Nom prévisible sans `O_EXCL`
   (lien symbolique possible). Piste : cache unique indexé par hash, ou
   fichier temporaire avec garde RAII.
3. Options : `-o`/`--src` sans valeur ignorés (sortie `out`), options
   inconnues acceptées, deux fichiers → le dernier gagne,
   `ocara --check` sans fichier compile `test.oc`, `ocara` sans argument sort
   en 0 (`cli.rs` ~l.108-166).
4. BOM UTF-8 refusé (« unexpected character » en 1:1) ; erreurs de lecture
   rapportées en `0:0`.

## Priorité / Complexité

**Haute** (perte de données, `/tmp` saturé) — **Simple**.

## Fichiers clés

`src/core/cli.rs`, `src/main.rs`, `src/codegen/link.rs`, `src/core/source.rs`.
