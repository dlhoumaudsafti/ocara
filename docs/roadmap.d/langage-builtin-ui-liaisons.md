# `ocara.UI` : liaisons déclaratives dans les deux sens

## Demande (David)

Lier une propriété d'un nœud du `.oc.ui` à un champ d'un objet Ocara, mise à
jour dans les deux sens :

- **code → interface** : `user.name = "Alice"` suffit, le nœud lié se
  réaffiche sans `ui.set` ;
- **interface → code** : l'utilisateur tape dans un `input`, `user.name`
  contient la saisie sans `ui.get`.

```yaml
- { type: input, id: name, bind: user.name }
- { type: checkbox, bind: user.newsletter }
- { type: text, bind: user.name, binds: { visible: user.active } }
```

```ocara
var user:User = use User("Alice")
ui.bind("user", user)   // la racine `user` du .oc.ui désigne cet objet
```

Le contrôleur ne contient plus que de la logique métier : les
`ui.get("search", "value")` / `ui.set("car_title", "value", …)` disparaissent.

## État actuel (vérifié dans le code)

- L'état des nœuds est **impératif** : `ui.get`/`ui.set`
  (`runtime_ui/src/lib.rs`, `get`, `set`, `set_json`). Le runtime garde une
  table `{ id: { prop: valeur } }` (`state`) ; `get` ne questionne pas
  l'interface.
- `bind:` et `binds:` existent, mais **seulement dans un gabarit `item:` de
  liste** (`src/core/ui_schema.d/nodes.rs`, compteur `in_item` ; ailleurs,
  « 'binds' n'est permis que dans le gabarit 'item' d'une liste »). Ils lisent
  un champ des `map` fournies par `ui.setItems`, à sens unique.
- Une saisie de l'utilisateur arrive dans `post` comme `{ id, prop, value }`
  et est rangée directement dans `state` **sur le thread du moteur**, sans
  passer par la file des actions.
- Les gestionnaires `ui.on` tournent **l'un après l'autre sur un thread Ocara
  dédié** (`QUEUE` et `dispatch` dans `runtime_ui/src/lib.rs`).
- Le runtime **ne sait pas lire un champ d'objet par son nom** : un objet est
  `[desc][rc][class_id][TAG_OBJECT][champs]`, et `desc` ne dit que quels
  champs sont sur le tas (`runtime/src/rc.rs`, `alloc_object`). `JSON_encode`
  ne traite que les `array`/`map`.
- Les appels `ui.get`/`ui.set`/`ui.on`… avec un littéral sont vérifiés à la
  compilation contre l'arbre du `.oc.ui` (`src/core/ui_file.rs`,
  `check_call`), mais au désucrage, **sans les types**.

## Spécification proposée

### Côté `.oc.ui`

- `bind: racine.champ[.champ…]` hors d'un gabarit `item:` lie la propriété
  principale du nœud (`value` d'un `input`/`text`, `checked` d'une
  `checkbox`/`switch`, `value` d'un `slider`…). `binds: { prop: racine.champ }`
  lie d'autres propriétés (`visible`, `enabled`, `color`…).
- Dans un gabarit `item:`, `bind: champ` garde son sens actuel (champ de
  l'élément). Un chemin qui commence par une racine liée (`user.…`) lit
  l'objet lié.
- Les racines ne sont pas déclarées dans le `.oc.ui` : elles sont déduites
  des chemins (`user` dans `user.name`). La liste des racines est ajoutée à
  l'arbre normalisé (`"roots": { "user": ["name", "newsletter", "active"] }`)
  pour le compilateur.

### Côté Ocara

- `ui.bind(racine, objet)` : relie la racine à un objet (classe utilisateur).
  Relier à nouveau la racine remplace l'objet, et tous les nœuds liés sont
  réaffichés. `ui.unbind(racine)` coupe le lien.
- `ui.refresh()` : force la comparaison décrite plus bas, pour un objet
  modifié hors d'un gestionnaire (timer, thread, `async`).

### Vérification à la compilation

Elle se fait dans `sema`, pas au désucrage : le type de l'objet passé à
`ui.bind` doit être connu.

- `racine` littérale absente du `.oc.ui` → erreur, avec suggestion.
- Chaque chemin de la racine doit désigner un champ **public** de la classe
  (en suivant les champs objets intermédiaires), sinon erreur à la ligne du
  `ui.bind`, avec le chemin et le nœud du `.oc.ui`.
- Type du champ compatible avec la propriété liée : `checked`/`visible`/
  `enabled` → `bool` ; `value` d'un `slider` → `int`/`float` ; `value` d'un
  `input`/`text` → `string`, ou nombre (converti, voir plus bas) ; `color` →
  `string`.
- Un champ intermédiaire nullable (`user.address.city` avec
  `address:Address|null`) : la lecture donne `null`, et une écriture de
  l'interface est ignorée tant que l'intermédiaire est `null`.
- Racine non littérale, ou `ui:UI` reçu en paramètre : vérification à
  l'exécution (`UIException`, prochain code libre).

### Code → interface : comparaison après chaque gestionnaire

Le runtime ne voit pas les écritures de champ (pas de Proxy ni de GC). La
v1 ne touche pas au codegen des écritures :

1. À la fin de chaque passage de `dispatch` (tous les gestionnaires d'une
   action exécutés), le runtime relit chaque chemin lié, le compare à la
   dernière valeur envoyée au moteur et n'envoie que les différences
   (même message qu'un `ui.set`).
2. Même comparaison après `ui.bind`, au premier affichage (`run`) et sur
   `ui.refresh()`.
3. Un objet modifié depuis un autre thread : `ui.refresh()` obligatoire.
   La relecture se fait sur le thread des actions (file `QUEUE`) pour ne pas
   lire un objet pendant qu'un gestionnaire l'écrit.

Le coût est proportionnel au nombre de chemins liés de l'écran, pas à la
taille du programme. Une notification à l'écriture (drapeau dans l'en-tête
de l'objet, testé par le codegen à chaque écriture de champ) ou un type
`State<T>` restent possibles plus tard si la comparaison coûte trop sur de
gros écrans : à mesurer, pas à présumer.

### Interface → code

- Une saisie sur un nœud lié (`{ id, prop, value }` dans `post`) est rangée
  dans `state` comme aujourd'hui, **et** une écriture est mise dans la file
  des actions. L'objet n'est donc écrit que sur le thread des actions : jamais
  pendant qu'un gestionnaire tourne.
- Les écritures en file sont appliquées **avant** l'action suivante : un
  `onClick: save` qui suit une saisie voit `user.name` à jour.
- Conversion vers le type du champ : `string` tel quel ; `int`/`float` en
  analysant le texte. Une saisie invalide (`"12a"` pour un `int`) ne modifie
  pas le champ ; le nœud garde le texte saisi, et un événement `invalid`
  (action déclarée par `onInvalid:` sur le nœud) permet d'afficher une
  erreur. À trancher : ignorer en silence si `onInvalid` est absent, ou
  lever `UIException` sur le thread des actions (signalée sur la sortie
  d'erreur comme une exception non rattrapée).
- **Pas d'écho** : une valeur venue de l'interface est enregistrée comme
  « dernière valeur envoyée » avant l'écriture dans l'objet. La comparaison
  suivante ne la renvoie donc pas au moteur, et le curseur de l'`input`
  reste en place.

### Accès aux champs : générés par le compilateur

Le runtime ne connaît pas les noms de champs : le compilateur génère, pour
chaque `ui.bind("racine", objet)` vérifié, une table
`chemin → (lecture, écriture)` de fonctions typées. La lecture rend le champ
en `mixed`. L'écriture convertit, contrôle le type et pose le champ en
respectant le refcount (`__rc_keep`, comme une affectation de champ). Pas de
réflexion à l'exécution, et le même mécanisme sert sur desktop et sur
Android (`runtime_ui` est commun). Le serveur de langage peut s'en servir
pour la définition et le renommage d'un champ depuis le `.oc.ui`.

### Mémoire

- L'UI retient l'objet lié (refcount +1), relâché par `unbind`, par un
  nouveau `bind` sur la racine ou par la libération de l'UI (`release`).
- Un objet qui garde le `ui` dans un champ forme un cycle : il est couvert par
  le détecteur de cycles. À tester avec `--leak-check`.
- Les valeurs relues pour la comparaison (chaînes) sont relâchées après
  l'envoi : aucune fuite par comparaison, à vérifier sur une boucle de
  saisies.

## Implémentation (pistes)

1. **Schéma** (`src/core/ui_schema.d/nodes.rs`, `schema.rs`) : accepter
   `bind`/`binds` hors d'un gabarit quand le chemin a une racine ; collecter
   `roots` dans l'arbre normalisé ; propriété principale de chaque type de
   nœud.
2. **Sema** : vérifier `ui.bind` (racine, chemins, visibilité, types) ; codes
   d'erreur dans `docs/diagnostics.md`.
3. **Abaissement** : générer la table de lecture et d'écriture passée à un
   nouveau `UI_bind(this, racine, objet, table)`.
4. **`runtime_ui/src/lib.rs`** : racines liées, dernière valeur envoyée par
   chemin, comparaison en fin de `dispatch`, écritures de l'interface mises en
   file, `refresh`, `unbind`.
5. **Moteurs** : aucun changement attendu (ils reçoivent des messages
   `set` ordinaires et envoient déjà `{ id, prop, value }`). À vérifier pour
   les nœuds liés dans un composant (`components:`) et dans les variantes de
   taille.
6. **Déclaration du builtin** (`src/builtins/ui.rs`, `src/codegen/desc.d/ui.rs`)
   et documentation (`docs/oc-ui.md`, `docs/builtins/UI.md`).
7. **Exemple** : refaire un formulaire de `mini_project_ui_hexa` avec des
   liaisons, à la place de ses `ui.get`/`ui.set`.

## Tests

- Saisie → champ, champ → nœud, sans écho (curseur conservé).
- Saisie suivie d'un clic : le gestionnaire voit la valeur saisie.
- Conversion `int`/`float`, saisie invalide.
- Chemin à champ intermédiaire `null`.
- `bind` remplacé, `unbind`, libération de l'UI : `--leak-check` à zéro.
- Erreurs de compilation : racine absente, champ absent, champ privé, type
  incompatible.
- `ui.refresh()` depuis un thread.
