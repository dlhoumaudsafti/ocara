# Comptage de références atomique + détecteur de cycles

Décision (2026-10-05) : toute valeur tas d'Ocara est comptée. Les comptes sont
atomiques partout, et un détecteur de cycles rattrape les références
circulaires. Ce n'est pas un GC (pas de ramasse-miettes traçant à la Go/Java) :
la libération reste déterministe, au moment où le compte tombe à zéro.

Le comptage remplace la preuve statique de propriété : `element_escape`,
`object_owners`, `object_facts`, la libération automatique des `var` et le
clonage à l'échappement. Il supprime la fuite du conteneur d'objets réellement
partagé (voir [memoire-scoped-object-elements-leak.md](memoire-scoped-object-elements-leak.md)).

## En-tête uniforme

```
[rc: i64 @ val-24][aux: i64 @ val-16][tag: i64 @ val-8][données @ val]
```

| Valeur | `aux` | Remarque |
|---|---|---|
| string possédée (`TAG_STRING_OWNED`) | longueur | littéral `.rodata` (`TAG_STRING`) : jamais compté |
| array / map | drapeaux (bit 0 : éléments bruts) | `array<int>`, `map<K,float>`… |
| objet (`TAG_OBJECT`) | `class_id` (inchangé, lu à `-16` par le dispatch) | champs décrits par le registre de classes |
| closure (`TAG_FUNCTION`) | 0 | `{func, env}` |
| env de closure (`TAG_ENV`) | nombre de captures | captures = cellules comptées |
| exception (`TAG_EXCEPTION`) | 0 | `message`, `source` |
| primitif boxé | — | cellule `[rc][bits]`, `val = (cellule+8) | tag` |

Le mot `rc` porte le compte (bits 0–47), la couleur du détecteur
(bits 48–55) et le drapeau « dans le tampon des racines » (bit 56).

## Primitives runtime

- `__rc_retain(v)` / `__rc_release(v)` : sans effet sur `0`, un entier brut,
  un littéral. `release` à zéro libère récursivement :
  - éléments d'un array/map, sauf s'ils sont bruts ;
  - champs tas d'un objet, d'après le registre `__rc_register_class(id, n, masque)` ;
  - env et captures d'une closure.
- Le drapeau « éléments bruts » est posé par le compilateur à la création d'un
  conteneur à éléments `int`/`float`/`bool`, et par le runtime pour les
  conteneurs qu'il crée. Il empêche de suivre un entier comme un pointeur.

## Convention d'appel

- Un argument est **emprunté** : l'appelé ne libère pas ce qu'il reçoit, et
  le retient s'il le stocke.
- Une valeur retournée appartient à l'appelant (+1). Un getter runtime
  (`__array_get`, `__map_get`, `first`…) retient l'élément qu'il rend.
- Un stockage (variable, champ, élément, capture) possède une référence :
  il retient une valeur empruntée et relâche l'ancienne valeur.
- Une sortie de portée (fin de bloc, `return`, `break`, `continue`) relâche
  les locales. Un temporaire non stocké est relâché après l'instruction.

## Détecteur de cycles

Collecte synchrone par suppression d'essai (Bacon–Rajan). Un `release` qui
laisse un conteneur, un objet ou une closure à un compte non nul l'ajoute au
tampon des racines. La collecte (marquage gris, balayage, ramassage des
blancs) se déclenche :

- quand le tampon dépasse un seuil ;
- seulement si aucun thread Ocara secondaire ne tourne (`Thread::run`,
  workers HTTPServer, `async`) : marquer pendant qu'un autre thread modifie
  des comptes serait faux ;
- à la sortie du programme.

## Limites connues

- `raise` traverse les frames par `longjmp` : les locales des frames sautées
  ne sont pas relâchées (fuite, jamais de libération prématurée).

## Phases

1. **Runtime** : en-tête, `__rc_retain`/`__rc_release`, drapeau brut,
   registre de classes, détecteur de cycles, tests runtime. Aucun changement
   de comportement.
2. **Compilateur** : enregistrement des classes au démarrage, drapeau brut
   posé à la création des conteneurs.
3. **Runtime + compilateur ensemble** :
   - getters qui retiennent, stockages qui retiennent et relâchent ;
   - émission des `retain`/`release` dans le lowering ;
   - suppression de la preuve statique ;
   - `scoped` = relâché en fin de portée, `consumed` = déplacement.
4. **Closures, cellules, exceptions, threads, handlers HTTP** : comptés.
5. **Cycles** : déclenchement, compteur de threads actifs, tests de cycles
   (parent ↔ enfant, auto-référence).
6. **Docs** : `docs/EBNF.md` (section mémoire), mesures de fuite, pages hexa.

## État d'avancement (2026-10-08)

**Phases 1 à 3 faites** :

- Runtime : `runtime/src/rc.rs` (en-tête, `__rc_retain`/`__rc_release`,
  détecteur de cycles, `ThreadGuard` sur `Thread::run`, `async` et workers
  HTTP). Insertions qui retiennent, écrasements et `Map::remove` qui
  relâchent, copies (`reverse`, `slice`, `sort`, `values`, `merge`) qui
  retiennent, insertions internes de valeurs neuves transférées
  (`array_push_owned`, `map_set_owned_key`), passages d'argument tel quel
  retenus. `__range`, octets de fichier/HTTP, sessions : conteneurs bruts.
- Objets : `[desc][rc][class_id][tag]`, `desc` = masque littéral des champs
  tas (`src/lower/builder.d/rc_layout.rs`).
- Lowering : `src/lower/stmt.d/rc.rs` (temporaires par instruction,
  portées, paramètres, boucles, conditions, bras de `match`, `return`,
  stockages hors locales). `consumed` relâchée après son premier usage.
- Preuve statique supprimée (`element_escape`, `object_owners`,
  `object_facts`, `class_ownership`, libération auto des `var`, clonage à
  l'échappement, fonctions runtime `__value_free`/`_clone`/`_shallow`/
  `_concrete`/`_objects`). `ownership.rs` ne ferme plus que les ressources.
- Sémantique (option A, choisie le 2026-10-08) : `var y = x` partage la
  valeur ; `scoped`/`consumed` rendent leur référence. Diagnostic E26 retiré.

Mesures (20 000 puis 200 000 appels, boucle `while`) : conteneur partagé
par deux porteurs, `match`, boucle de concaténation, objets : stables
(≈ 2 à 3 Mo).

Tests : `examples/tests/84_refcountTest.oc`, `runtime/src/tests/rc.rs`.

## Reste à faire

- **Phase 4 — cellules et closures comptées** : une variable capturée
  (closure, corps et gestionnaires de `try`) vit dans une cellule jamais
  libérée, sa valeur n'est jamais relâchée en fin de portée ; envs de
  closure, valeurs par défaut, envs `async` jamais libérés. Serveur
  `mini_project_hexa` : ≈ 15 Ko perdus par requête (≈ 45 Ko avant le
  comptage).
- Générateurs : aucune libération (`rc_no_release`).
- Fuites runtime relevées par l'audit : `IO_readInt`/`Float`/`Bool`/
  `Array`/`Map` (chaîne lue), `call_component` (attributs et résultat),
  `throw_*` (`type_name`), `map_get_*` SDL/Tauri.
- Valeurs non taguées rendues comme valeurs Ocara (`HTTPRequest_*`,
  `SQLite_open`, `MySQL_connect`…) : jamais comptées par leur type, mais un
  tel handle rangé dans un `mixed` n'est ignoré par `rc::kind` que grâce à
  l'en-tête de bloc de glibc — à revoir pour Android/Windows.
