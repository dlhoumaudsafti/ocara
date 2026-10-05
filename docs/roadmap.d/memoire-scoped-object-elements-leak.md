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

## Étape 2 — cas restants couverts

Preuve statique sur tout le programme (`src/lower/stmt.d/object_facts.rs`,
point fixe en deux phases) avec un parcours commun
(`object_owners.rs`, `Scan`) :

- **Conteneur issu d'un appel** (`scoped items = all()`) : `fresh_containers`,
  fonctions qui ne retournent que des conteneurs neufs (littéral d'objets
  neufs, appel d'une telle fonction, conteneur local propriétaire retourné).
- **Conteneur passé en argument** (`fill(items)`) : `preserving_params`,
  paramètres dont l'appelé ne garde rien et n'insère que des objets neufs ;
  un tel appel est un simple prêt.
- **Champs `array<Classe>`/`map<K, Classe>`** : `owning_fields`, champ (par
  nom) dont toutes les valeurs entrantes sont neuves :
  - littéral, appel qui retourne un conteneur neuf ;
  - conteneur local **déplacé**, une seule fois et sinon seulement rempli par
    `push` ;
  - paramètre dont **tous** les sites d'appel passent un conteneur neuf.
    Les appels `obj.m(...)`/`parent::m(...)` sont rapprochés par nom de
    méthode ; une fonction référencée comme valeur ne reçoit jamais de
    transfert.

  Tous les accès du programme à ce nom de champ doivent par ailleurs être des
  lectures, sans élément conservé. `__free_<Classe>` libère alors ses objets
  (`__array_free_objects`), `__clone_<Classe>` les clone.

**Use-after-free corrigé au passage** : `__free_<Classe>` libérait toujours le
tableau d'un champ conteneur d'objets, même partagé (`use Bag(ys)`, puis
`ys[0]` relu après la libération de l'objet : SIGSEGV). Un champ non
propriétaire n'est plus libéré ni dupliqué par l'objet.

Mesures (20 000 puis 200 000 appels) : `scoped items = all()`,
`fill(items)` et `scoped bag = use Bag(all())` stables à ≈ 1,95 Mo.

## Reste ouvert (fuite, jamais de double libération)

- Valeur obtenue par `resolve` d'un appel `async` (`CarDetailsDTO.maintenances`
  dans `mini_project_hexa` : `forCar` est `async`).
- Analyse des champs par NOM : un seul accès douteux à un champ `items` de
  n'importe quelle classe disqualifie tous les champs `items`. Un champ
  disqualifié n'est plus libéré du tout, alors qu'il l'était, au risque d'un
  use-after-free.
- Conteneur déplacé vers un champ puis relu localement : refusé par
  prudence, l'ordre des usages n'étant pas suivi.

Tests : `examples/tests/81_object_ownership_transfersTest.oc`, tests
unitaires de `object_facts.rs`.

Tests : `examples/tests/80_scoped_object_containersTest.oc`, tests
unitaires de `object_owners.rs`.

## Fichiers clés

`runtime/src/lib.rs`, `src/codegen/desc.d/lowlevel.rs`,
`src/lower/stmt.d/object_owners.rs`, `src/lower/stmt.d/ownership.rs`,
`src/lower/stmt.d/element_escape.rs` (`prepare_body`),
`src/lower/builder.d/program.rs`, `src/lower/builder.d/functions.rs`.
