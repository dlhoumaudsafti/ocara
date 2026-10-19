# Builtins sans type de retour déclaré : `int`/`bool` lus comme `mixed` (SIGSEGV, `null`, fuites)

Audit du 2026-10-19. Reproduit sur le binaire courant.

## Constat

La table manuelle `fn_ret_types` (`src/lower/builder.d/program.rs`,
~l.760-830) n'a pas d'entrée pour plusieurs builtins. Leur résultat est alors
traité comme `mixed` à l'affichage ou dans un gabarit :

- `IO::writeln(File::size("f"))` sur un fichier de 100 000 octets → SIGSEGV ;
- `${Thread::currentId()}` → `null` au lieu de `0` ;
- `${res.status()}` → `null` ; `${res.ok()}`, `${m.tryLock()}` → `1`/`null`
  au lieu de `true`/`false` ;
- `${r.error()}`, `${r.body()}` sur un `HTTPResponse` → une chaîne perdue par
  appel (`--leak-check`).

Passer par une variable typée (`var z:int = File::size(...)`) contourne le
problème. Symboles concernés (int/bool) : `File_exists`, `File_size`,
`Directory_exists`, `Directory_count`, `Thread_id`, `Thread_currentId`,
`Mutex_tryLock`, `HTTPRequest_status`, `HTTPRequest_ok`,
`HTTPRequest_isError`, `HTTPServerSession_has`, `HTTPServerSession_hasGlobal` ;
d'autres symboles à retour pointeur manquent aussi (`File_read`,
`HTTPRequest_body`…).

## Piste

Dériver `fn_ret_types` des descripteurs `BuiltinDesc` (`src/codegen/desc.d/`)
et des signatures de `src/builtins/` au lieu d'une table manuelle, et ajouter
un test qui vérifie que chaque builtin a un type de retour.

## Priorité / Complexité

**Haute** (crash, fuite) — **Légère**.

## Fichiers clés

`src/lower/builder.d/program.rs`, `src/codegen/desc.d/*.rs`, `src/builtins/*.rs`.
