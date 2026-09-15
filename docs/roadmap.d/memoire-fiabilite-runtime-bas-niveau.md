# Fragilités bas niveau du runtime mémoire : deux points restants

Le tag d'exception confondu avec `TAG_MAP`, la portabilité `Mutex`, `free_str` (taille recalculée), le SEGFAULT `read_tag` sur un entier `mixed`/concret, le boxing des arguments de méthode, et `UnitTest::assertContains` (gardé contre un argument boxé, même correctif qu'`assertEmpty`/`assertNotEmpty`) sont tous corrigés — voir git log pour le détail (c'est un chantier long, plusieurs bugs latents plus larges que prévu y ont été découverts et corrigés au passage).

## Reste à faire : une string à NUL interne reste tronquée à l'affichage/comparaison

`free_str` connaît maintenant la vraie longueur d'une string possédée (plus de risque de corruption à la libération), mais `ptr_to_str` (donc l'affichage, la comparaison, `String::*`, `JSON::encode`...) reste basé sur `CStr::from_ptr`, qui tronque toujours au premier octet NUL. Une string Ocara contenant un `\0` interne (échappement valide) reste donc affichée/comparée tronquée partout dans le langage, même si elle ne corrompt plus la mémoire. Corriger ça de bout en bout demanderait une représentation de string à longueur explicite (pas seulement NUL-terminée) — un changement de représentation plus large que ce qui a été fait jusqu'ici.

## Reste à faire (mineur, non confirmé en pratique) : conteneur imbriqué à deux niveaux et libération "shallow"

La libération/le clonage "shallow" d'un `array<T>`/`map<K,T>` à élément primitif concret (évite de traiter chaque élément comme un pointeur potentiel) ne couvre que le niveau immédiat de chaque `var`/`scoped`/`consumed`. Un conteneur imbriqué (`array<array<int>>`) reste sur le chemin récursif générique au niveau externe — un tableau interne `array<int>` atteint par cette récursion resterait exposé au risque d'origine. Jamais rencontré dans aucun exemple existant, mais pas prouvé impossible.

## Fichiers clés

`runtime/src/lib.rs` (`ptr_to_str`, `__array_free_shallow`/`__map_free_shallow`), `src/lower/stmt.d/ownership.rs` (`drop_func_for`/`clone_func_for`).
