# Fuite : objets d'un conteneur `scoped`/`consumed` jamais libérés

## Constat

Mesuré en vérifiant le correctif de
[langage-hexa-car-details-bugs](langage-hexa-car-details-bugs.md) (mémoire
maximale pour 20 000 puis 200 000 appels) :

```ocara
function repo(): int {
    consumed rows:array<map<string, mixed>> = JSON::decode("[{\"name\":\"alpha\"},{\"name\":\"beta\"}]")
    scoped items:array<Item> = []
    for row in rows {
        items.push(fromRow(row))
    }
    return items.len()
}
```

2,1 Mo puis 24,5 Mo : la mémoire croît avec le nombre d'appels. Les mêmes
mesures sur des `string`/`array`/`map` imbriqués restent stables (≈ 2 Mo).

## Cause

`scoped items:array<Item>` est libéré par `__value_free` (chemin générique,
`value_ownership_symbol`). Ce dernier libère chaînes, tableaux et maps, mais
ignore une instance de classe (`TAG_OBJECT`, voir le commentaire « Portée :
string/array/map uniquement » dans `runtime/src/lib.rs`) : le runtime ne
connaît pas `__free_<Classe>`, généré par le compilateur. Les objets et leurs
champs fuient.

## Pistes

- **Registre** `class_id → (__free_<Classe>, __clone_<Classe>)`, rempli au
  démarrage du programme (le `class_id` est déjà dans l'en-tête de chaque
  instance, `*(val - 16)`). `__value_free`/`__value_clone` dispatchent dessus.
  Libération et clone vont ensemble : `maybe_clone_escaping` clone le
  conteneur qui s'échappe, sans quoi la copie pointerait vers des objets
  libérés.
- **Partage** : un objet poussé dans un conteneur `scoped` et encore
  référencé ailleurs (`var x = use Item(); items.push(x)`, puis `x` utilisé
  après la libération de `items`) deviendrait un use-after-free. Ce cas
  existe déjà pour les chaînes poussées. Il faut que l'analyse
  d'échappement le refuse, ou qu'un `push` d'une valeur conservée ailleurs
  la copie.
- Ou, sans registre : pour un `array<Classe>`/`map<K, Classe>` de type
  statique connu, le lowering émet une boucle `__free_<Classe>` sur les
  éléments puis `__array_free_shallow`.

## Priorité / Complexité

Haute (fuite) — **Structurel, Dangereuse**.

## Fichiers clés

`runtime/src/lib.rs` (`__value_free`, `__value_clone`, `__object_free`),
`src/lower/stmt.d/ownership.rs` (`value_ownership_symbol`),
`src/lower/builder.d/class_ownership.rs`.
