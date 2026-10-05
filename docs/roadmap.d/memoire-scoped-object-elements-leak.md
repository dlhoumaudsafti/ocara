# Objets d'un conteneur `scoped`/`consumed` — corrigé (cas prouvés)

## Constat

`scoped items:array<Item>` ne libérait que le tableau : `__value_free` ignore
les instances (`TAG_OBJECT`), faute de connaître `__free_<Classe>`. Mesuré :
mémoire maximale de 2,1 Mo pour 20 000 appels, 24,5 Mo pour 200 000.

## Correctif

- **Runtime** : `__array_free_objects`/`__map_free_objects` et
  `__array_clone_objects`/`__map_clone_objects` (`runtime/src/lib.rs`)
  reçoivent l'adresse de `__free_<Classe>`/`__clone_<Classe>`
  (`Inst::FuncAddr`). Pas de registre global : le type d'élément est connu
  statiquement.
- **Propriété prouvée** (`src/lower/stmt.d/object_owners.rs`) : un conteneur
  libère ses objets seulement si CHAQUE objet qui y entre est neuf :
  - `use Classe(...)` ;
  - appel d'une fonction ou méthode qui ne retourne que des objets neufs
    (`compute_fresh_returns`, point fixe, `IrModule::fresh_returns`) ;
  - variable initialisée ainsi et référencée nulle part ailleurs.

  Sont disqualifiés : un initialiseur non littéral, la réaffectation, le
  passage en argument, toute méthode autre que `push`/`len`, et la capture
  par une closure. Un élément extrait et conservé fait déjà passer le
  conteneur en libération de surface (`element_escape`).
- **Ownership** (`ownership.rs`) : `OwnedLocalInfo.object_class` →
  `OwnershipFunc::Objects`.

Mesure : `repo()` (`push(fromRow(row))` dans un `scoped array<Item>`) reste
stable à 1,9 Mo pour 20 000 comme pour 200 000 appels.

## Bug corrigé au passage

`xs.push(...)` sur un **paramètre** `array<Classe>` était compilé en
`String_push` (SIGSEGV) : les paramètres `string`/`array`/`map` n'avaient pas
de classe builtin dans `var_class`. Ils passent maintenant par
`register_var_class`, comme une variable locale (`functions.rs`).

## Reste ouvert (fuite, jamais de double libération)

- Conteneur initialisé par un appel (`scoped items = repo.all()`) : on ne
  sait pas si l'appelé a gardé des références à ses objets.
- Conteneur passé en argument : l'appelé peut y insérer des objets partagés.
- Champs `array<Classe>`/`map<K, Classe>` d'une classe : `__free_<Classe>`
  libère le tableau, pas ses objets.

Pistes : propager « retourne un conteneur neuf d'objets neufs » comme
`fresh_returns`, et appliquer la même preuve aux champs (constructeur et
méthodes de la classe).

Tests : `examples/tests/80_scoped_object_containersTest.oc`, tests
unitaires de `object_owners.rs`.

## Fichiers clés

`runtime/src/lib.rs`, `src/codegen/desc.d/lowlevel.rs`,
`src/lower/stmt.d/object_owners.rs`, `src/lower/stmt.d/ownership.rs`,
`src/lower/stmt.d/element_escape.rs` (`prepare_body`),
`src/lower/builder.d/program.rs`, `src/lower/builder.d/functions.rs`.
