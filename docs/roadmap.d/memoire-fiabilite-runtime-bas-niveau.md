# Fragilités bas niveau du runtime mémoire : un point restant

Le tag d'exception confondu avec `TAG_MAP`, la portabilité `Mutex`, `free_str` (taille recalculée), le SEGFAULT `read_tag` sur un entier `mixed`/concret, le boxing des arguments de méthode, `UnitTest::assertContains` (gardé contre un argument boxé), et la troncature au premier NUL interne d'une string (littéraux ET strings possédées partagent maintenant le même header `[len][tag]`, lu directement par `ptr_to_str` au lieu d'un scan `CStr::from_ptr`) sont tous corrigés — voir git log pour le détail (c'est un chantier long, plusieurs bugs latents plus larges que prévu y ont été découverts et corrigés au passage).

## Reste à faire (mineur, non confirmé en pratique) : conteneur imbriqué à deux niveaux et libération "shallow"

La libération/le clonage "shallow" d'un `array<T>`/`map<K,T>` à élément primitif concret (évite de traiter chaque élément comme un pointeur potentiel) ne couvre que le niveau immédiat de chaque `var`/`scoped`/`consumed`. Un conteneur imbriqué (`array<array<int>>`) reste sur le chemin récursif générique au niveau externe — un tableau interne `array<int>` atteint par cette récursion resterait exposé au risque d'origine. Jamais rencontré dans aucun exemple existant, mais pas prouvé impossible.

## Fichiers clés

`runtime/src/lib.rs` (`ptr_to_str`, `__array_free_shallow`/`__map_free_shallow`), `src/lower/stmt.d/ownership.rs` (`drop_func_for`/`clone_func_for`).
