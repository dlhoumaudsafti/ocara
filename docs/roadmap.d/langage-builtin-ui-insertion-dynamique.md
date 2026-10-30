# `ocara.UI` : insérer des nœuds depuis le code

## Demande (David)

Pouvoir créer depuis le code un élément qui n'est pas écrit dans le `.oc.ui`
(par exemple un bouton), et insérer dans un conteneur un élément de liste ou
une instance d'un composant déclaré.

```ocara
var id:string = ui.insert("toolbar", "button", {"text": "Nouveau", "onClick": "add_car"})
ui.insert("cards", "carCard", {"title": "Clio", "price": "9 000 €", "onOpen": "open_car"})
ui.remove(id)
```

## État actuel (vérifié dans le code)

- L'arbre du `.oc.ui` est validé **à la compilation** et embarqué dans le
  binaire. L'API n'a aucune méthode pour ajouter ou retirer un nœud
  (`src/builtins/ui.rs`).
- Le seul affichage dynamique d'un nombre variable d'éléments est
  `ui.setItems(id, rows)` : une `list` répète son gabarit `item:`, et il faut
  renvoyer **tout** le tableau à chaque changement.
- `ui.component("carCard")` ne crée pas d'instance : il lie les actions
  propres d'un composant pour ses instances déclarées.
- La validation d'un nœud (types de nœud, propriétés, paramètres de
  composant) vit dans le compilateur (`src/core/ui_schema.d/`). Le runtime
  (`runtime_ui/`) ne la connaît pas.
- L'arbre normalisé contient `ids` (id → type de nœud) et `actions` (actions
  déclarées) (`src/core/ui_schema.d/mod.rs`). `ui.on` refuse une action
  absente de `actions` (`UIException` 101), et `check_call`
  (`src/core/ui_file.rs`) vérifie à la compilation les `id` littéraux de
  `get`/`set`.
- L'état posé par le code est gardé par le runtime (`state`) et rejoué quand
  la page ou l'Activity est recréée (`OcaraUI.restore`).

## Spécification proposée

### 1. Opérations élément par élément sur une `list`

Pour ne plus renvoyer tout le tableau :

| Méthode | Rôle |
|---|---|
| `ui.addItem(list, row)` | ajoute un élément à la fin |
| `ui.insertItem(list, index, row)` | insère au rang `index` |
| `ui.updateItem(list, index, row)` | remplace l'élément du rang `index` |
| `ui.removeItem(list, index)` | retire l'élément du rang `index` |

Un rang hors limites lève une `UIException` (prochain code libre). Seul
l'élément concerné est envoyé au moteur.

### 2. Insertion d'un nœud ou d'une instance dans un conteneur

| Méthode | Rôle |
|---|---|
| `ui.insert(container, type, props)` → `string` | ajoute un nœud à la fin du conteneur et rend son `id` |
| `ui.insertAt(container, index, type, props)` → `string` | idem, au rang `index` parmi les enfants |
| `ui.remove(id)` | retire un nœud inséré (un nœud du `.oc.ui` : `UIException`) |
| `ui.clear(container)` | retire tous les nœuds insérés du conteneur |

- `container` : `id` d'un nœud qui a des `children` (`column`, `row`,
  `stack`, `grid`, `card`, `androidPanel`…). Une `list` est refusée : elle a
  ses propres méthodes (§1).
- `type` : un type de nœud (`"button"`, `"text"`, `"input"`…) ou un
  **composant déclaré** dans `components:` (`"carCard"`).
- `props` : `map<string, mixed>`, avec les propriétés du nœud ou les
  paramètres du composant. `children` est permis pour un conteneur inséré,
  validé de la même façon.
- `id` : celui de `props["id"]` s'il est donné (un `id` déjà présent lève une
  `UIException`), sinon un `id` généré (`<container>.<n>`). Il sert à `get`,
  `set`, `remove`, et, pour une instance, à `event.instance()`.
- Les nœuds insérés font partie de l'état gardé : ils sont rejoués quand la
  page ou l'Activity est recréée, et conservés par le rechargement à chaud
  tant que leur conteneur existe.
- Un conteneur présent dans plusieurs variantes de taille garde ses nœuds
  insérés d'une variante à l'autre (ils sont rattachés à l'`id`, pas à la
  variante).

### Actions d'un nœud inséré

`ui.on` n'accepte qu'une action déclarée dans le `.oc.ui`. Une action
utilisée uniquement par des nœuds insérés se déclare dans une nouvelle clé
racine :

```yaml
actions: [add_car, open_car]
```

Un `onClick` (ou toute propriété de type action) d'un nœud inséré qui ne
désigne pas une action déclarée lève une `UIException`. Les actions propres
d'un composant inséré fonctionnent comme pour une instance déclarée
(`ui.component("carCard").on(…)`, `event.instance()` rend l'`id` inséré).

### Vérification

- **À la compilation**, quand `container`, `type` et `props` sont des
  littéraux (map littérale) : conteneur existant qui accepte des enfants,
  type connu, propriétés connues et de bon type, paramètres obligatoires du
  composant présents, actions déclarées. Mêmes messages et suggestions que
  pour le `.oc.ui`. Un `id` littéral donné dans `props` est ajouté aux `id`
  connus, pour que `ui.get("cet_id", …)` compile.
- **À l'exécution** pour le reste : même validation, `UIException` en cas
  d'erreur.

Pour que le runtime valide comme le compilateur sans recopier le schéma (une
seule source de vérité), sortir la validation d'un nœud de
`src/core/ui_schema.d/` dans une crate partagée par le compilateur et
`runtime_ui`. Autre option : embarquer dans l'arbre normalisé le schéma des
types de nœud et les paramètres des composants. À trancher en mesurant la
taille ajoutée au binaire et à l'APK.

## Implémentation (pistes)

1. **Schéma** : clé racine `actions:` ; validation d'un nœud isolé
   accessible au runtime (voir plus haut).
2. **Compilateur** : déclarations des nouvelles méthodes
   (`src/builtins/ui.rs`, `src/codegen/desc.d/ui.rs`) ; `check_call` pour les
   appels littéraux.
3. **`runtime_ui/src/lib.rs`** : nœuds insérés par conteneur dans l'état,
   génération des `id`, `remove`/`clear`, rejeu dans `state()` ; opérations
   sur les éléments des listes.
4. **Moteurs** : messages `insert`/`remove` et `addItem`/`insertItem`/
   `updateItem`/`removeItem` dans `runtime_tauri/src/ui/engine.js` et dans le
   moteur Compose (`packaging/android/app/src/main/java/com/ocara/bridge/OcaraUI*.kt`),
   en réutilisant le rendu d'un nœud existant ; dépliage d'une instance de
   composant au moment de l'insertion (même code que pour une instance
   déclarée).
5. **Documentation** : `docs/oc-ui.md` (clé `actions:`), `docs/builtins/UI.md`
   (méthodes, codes d'erreur).

## Tests

- Insertion d'un bouton puis clic : l'action arrive au gestionnaire.
- Insertion d'une instance de composant : paramètres appliqués, action
  propre avec `event.instance()`.
- `insertAt`, `remove`, `clear`, `id` en double, conteneur invalide, `remove`
  d'un nœud du `.oc.ui`.
- `get`/`set` sur un nœud inséré.
- Page ou Activity recréée, rechargement à chaud, changement de variante de
  taille : nœuds insérés conservés.
- Opérations sur les éléments d'une liste, rangs hors limites.
- Erreurs de compilation sur des appels littéraux invalides.
- `--leak-check` après insertions et retraits répétés.
