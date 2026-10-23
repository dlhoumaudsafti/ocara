# Types numériques de largeur explicite (`int8`…`uint64`, `float16`…`float64`)

Statut : **conception validée le 2026-10-23**, avant implémentation. La famille
complète est retenue ; `int` et `float` sont des alias de `int64` et
`float64` (représentation actuelle inchangée). La réalisation est massive et
sera menée en deux phases, décrites ci-dessous.

## Proposition étudiée

Entiers :

| Type | Plage de valeurs | Taille numérique |
|---|---|---|
| `int8` | −128 à 127 | 1 octet |
| `uint8` | 0 à 255 | 1 octet |
| `int16` | −32 768 à 32 767 | 2 octets |
| `uint16` | 0 à 65 535 | 2 octets |
| `int32` | −2 147 483 648 à 2 147 483 647 | 4 octets |
| `uint32` | 0 à 4 294 967 295 | 4 octets |
| `int64` | −9 223 372 036 854 775 808 à 9 223 372 036 854 775 807 | 8 octets |
| `uint64` | 0 à 18 446 744 073 709 551 615 | 8 octets |
| `int` | alias de `int64` | 8 octets |

Flottants :

| Type | Plage finie approximative | Taille |
|---|---|---|
| `float16` | −65 504 à 65 504 | 2 octets |
| `float32` | −3,4 × 10³⁸ à 3,4 × 10³⁸ | 4 octets |
| `float64` | −1,8 × 10³⁰⁸ à 1,8 × 10³⁰⁸ | 8 octets |
| `float` | alias de `float64` | 8 octets |

« Taille numérique » désigne la largeur de la valeur elle-même, pas
l'espace qu'elle occuperait réellement en mémoire : voir l'état actuel
ci-dessous, où tout occupe aujourd'hui un mot de 8 octets.

## État actuel (vérifié le 2026-10-18)

Vérifié dans le code et par exécution avec le compilateur courant :

- **Deux types numériques seulement** : `int` « entier signé 64 bits » et
  `float` « flottant IEEE 754 double précision » (`docs/EBNF.md` §6.1).
- **IR** : `int` → `IrType::I64`, `float` → `IrType::F64`, `bool` →
  `IrType::Bool` (`src/ir/types.rs`).
- **Codegen** : toute variable Cranelift est un mot de 64 bits — `I64`,
  `Bool`, `Ptr` et `Void` deviennent `I64`, `F64` reste `F64`
  (`src/codegen/emit.d/helpers.rs`). Les opérations flottantes travaillent
  sur des bits rangés dans un `I64`, convertis par `bitcast` vers `F64`
  autour de chaque `fadd`/`fsub`/`fmul`/`fdiv`
  (`src/codegen/emit.d/instructions.d/arithmetic.rs`).
- **Littéraux** : un littéral entier est lu en `i64` ; hors plage, c'est une
  erreur de compilation (`integer too large`). Un littéral flottant est lu en
  `f64` ; trop grand, il devient `inf` (`src/parsing/lexer.d/scanner.d/readers.rs`).
- **Dépassement entier** : `+`, `-`, `*` sont émis en `iadd`/`isub`/`imul`
  sans contrôle — `9223372036854775807 + 1` donne `-9223372036854775808`
  (bouclage en complément à deux), sans erreur.
- **Division entière** (corrigé le 2026-10-18) : `/` et `%` entre `int`
  passent par le runtime (`__int_div`/`__int_mod`) ; un diviseur nul lève une
  `MathException` (code 104), `INT_MIN / -1` boucle, `INT_MIN % -1` vaut 0
  (`docs/EBNF.md` §11). Une constante de classe calculée à la compilation
  rejette la division par zéro et le débordement (`docs/diagnostics.md`, E55).
- **Flottants** : `1.0 / 0.0` donne `inf` ; `0.1 + 0.2` donne
  `0.30000000000000004` (comportement IEEE 754 double).
- **Pas de conversion implicite** entre `int` et `float` : `var f:float = 3`
  (variable `int`), `var a:int = 2.9` et `a + f` sont des erreurs de
  compilation. Seule exception : les comparaisons, où `int`/`float` est la
  seule paire compatible, avec élargissement numérique (`10 equal 10.0` vaut
  `true`, `docs/EBNF.md` §11.1). Conversions explicites : `Convert` et
  `n.toFloat()`, `f.toInt()`… (`docs/builtins/Convert.md`).
- **Valeurs `mixed`** : un `mixed` est un mot de 64 bits ; un `float` y est
  toujours boxé (cellule allouée, bits bas `01`) ; un `int` y reste brut s'il
  est négatif ou compris entre 1 et 65 535, et est boxé sinon (bits bas `11`,
  `0` compris pour le distinguer de `null`) ; un `bool` y est boxé (`10`)
  (`runtime/src/typecheck.rs`, `runtime/src/lib.rs`).
- **Mémoire** : un champ d'objet occupe 8 octets quel que soit son type
  (décalage `index × 8`, `src/lower/expr.d/helpers.rs::field_offset`) ; un
  tableau stocke ses éléments dans un `Vec<i64>` (8 octets chacun, flottants
  rangés en bits ; drapeau « éléments bruts » int/float/bool sur le conteneur,
  `runtime/src/containers.rs`, `runtime/src/rc.rs`) ; une map stocke ses
  clés en texte (une clé `int`/`float`/`bool` est convertie en chaîne) et ses
  valeurs en mots de 8 octets.
- **Builtins** : l'ABI du runtime ne connaît que des mots `i64` et `f64`
  (descripteurs `BuiltinDesc`, `src/codegen/desc.d/`). Il n'existe pas de
  FFI exposée au code Ocara : « FFI » désigne seulement les appels du code
  généré vers le runtime Rust (`docs/workflow-compilation.md`).

## Décisions de conception validées (2026-10-23)

### `int` et `float` : alias de `int64` et `float64`

- Ajouter les huit types entiers fixes (`int8`…`int64`, `uint8`…`uint64`) et
  les trois flottants fixes (`float16`…`float64`).
- `int` est un alias de `int64`, `float` un alias de `float64` : même
  représentation qu'aujourd'hui (un mot de 64 bits), aucun coût ajouté aux
  boucles et aux calculs existants. `var x:int = 5` et `var x:int64 = 5` sont
  le même type ; `is int` et `is int64` sont équivalents.
- **Écarté** : un `int`/`float` « dynamique » qui choisirait la largeur
  minimale selon sa valeur. Sa plage (`int64::MIN` à `uint64::MAX`) demandait
  65 bits, donc un suivi signé/non signé à chaque opération sur tout le code
  `int` ; la largeur recalculée n'était observable que par `is`, qui
  devenait un test de plage déguisé (`255 is uint8` vrai, faux après `++`) ;
  un `float` dynamique imposait un aller-retour de conversion à chaque
  opération.
- Le compilateur n'émet **aucun avertissement** sur `int`/`float`. C'est
  `ocaracs` qui analyse les variables, champs, paramètres et collections
  `int`/`float` et propose un type plus étroit quand il prouve que toutes les
  valeurs tiennent dans sa plage (par exemple `uint16` pour une valeur
  toujours comprise entre 0 et 65 535). L'analyse suit les affectations,
  branches, boucles, appels et entrées externes ; une source qu'elle ne peut
  pas borner rend la conclusion inconnue, jamais faussement sûre. Le
  correctif est une suggestion à valider, avec la plage et les hypothèses
  établies.
- Les variables restent toujours annotées (`var x:int = 5`) ; `var x = 5` n'est
  pas une forme valide.
- Un littéral numérique sans contexte est un `int` (`int64`) ou un `float`
  (`float64`). Dans un contexte à type fixe (déclaration, argument, élément
  de collection typée), il est vérifié directement dans le type attendu ;
  une constante hors plage est une erreur de compilation.

### Tests de type

- Chaque type numérique a son test précis (`is int8`, `is uint8`,
  `is float16`…) : vrai seulement pour une valeur réellement de ce type
  (déclarée ou convertie), jamais selon sa valeur — un `int` qui vaut 5
  n'est pas un `uint8`.
- Prédicat de famille `is numeric`, vrai pour tous les types entiers et
  flottants, faux pour les autres, sans conversion des chaînes numériques :
  ```ocara
  var i:uint8 = 5
  var f:float16 = 3.14
  var s:string = "123"

  i is numeric // true
  f is numeric // true
  s is numeric // false
  ```
  `numeric` est une catégorie des tests `is`, pas un type utilisable en
  annotation ou en élément de collection.

### Entiers, opérations et conversions

- Le dépassement de tout entier lève `IntegerException`, `int` (`int64`)
  compris : `9223372036854775807 + 1` ne boucle plus. Coût à mesurer sur
  `make perf` (une instruction de test par opération, `loop` en tête) :
  aucune mesure ne doit devenir plus lente qu'un langage interprété.
- Entre types entiers de même signe, une opération promeut vers la largeur
  supérieure (`uint8 + uint16` → `uint16`), et le résultat est vérifié dans
  ce type ; une opération arithmétique signé/non signé est refusée à la
  compilation (conversion explicite). Les comparaisons entre largeurs et
  signes différents restent permises et exactes.
- Les affectations implicites n'autorisent que les élargissements qui
  préservent exactement toutes les valeurs source (`uint8` → `uint16`,
  `int32` → `int64`, `uint32` → `int64`). Une conversion rétrécissante ou
  signé/non signé non sûre passe par `Convert` ou une méthode d'instance
  (`.toUint8()`, etc.), vérifiée : hors plage, elle lève `IntegerException`,
  jamais de troncature ni de bouclage.
- `Convert` étend sa matrice de conversions numériques à toutes les
  largeurs, avec les méthodes d'instance correspondantes.

### Boucles à variable typée

Aujourd'hui, la variable d'une boucle sur une plage est toujours un `int` :
`for i in a to b`. Pour durcir le typage strict, ligne de conduite d'Ocara,
la boucle peut déclarer le type de sa variable :

```ocara
for i:uint8 in 0 to 255 { pixels.push(i) }        // array<uint8>, sans conversion
for i:int32 in start until stop { … }
```

- Sans type, la variable reste un `int` (compatibilité).
- Les bornes sont vérifiées dans le type déclaré : une borne constante hors
  plage est une erreur de compilation (`for i:uint8 in 0 to 256`) ; une
  borne calculée est vérifiée **une seule fois, à l'entrée de la boucle**
  (`IntegerException`), pas à chaque tour — toutes les valeurs de `i`
  restent entre les bornes, aucun contrôle par itération.
- Le compteur ne doit pas déborder sur la dernière valeur : `0 to 255` en
  `uint8` s'arrête après 255 sans calculer 256 dans le type (compteur sur
  64 bits en interne, ou test avant l'incrément).
- Pour une boucle sur une collection (`for v:uint8 in octets`), le type
  déclaré doit être celui des éléments ou un élargissement sûr ; sinon,
  erreur de compilation.
- Gain attendu : de typage (la variable s'utilise sans conversion là où un
  `uint8` est attendu), pas de vitesse — les registres restent de 64 bits.

### Flottants et comparaisons

- Les flottants fixes suivent IEEE 754 à leur précision déclarée ; un calcul
  `float16` est fait en `float32` puis arrondi (pas d'arithmétique `float16`
  matérielle sur x86, F16C ne fait que des conversions). Élargissement
  implicite `float16` → `float32` → `float64` seulement.
- Une conversion ou une opération qui sort de la plage finie d'un flottant
  fixe lève `FloatException`. Les valeurs spéciales IEEE restent définies :
  NaN n'est égal à aucune valeur, les zéros signés sont égaux, les infinis
  sont ordonnés.
- Les comparaisons entier/flottant et inter-largeurs sont exactes, sans
  conversion arrondie vers un flottant commun.
- Les clés flottantes de map sont canoniques : `+0.0` et `-0.0` désignent la
  même clé ; tous les NaN désignent une clé canonique.

### Exceptions et `mixed`

- `NumericException` est la classe de base ; `IntegerException` et
  `FloatException` en sont des sous-classes. `MathException` reste utilisée
  pour la division entière par zéro.
- `mixed` conserve le type réellement stocké (`m is uint8` après
  `var m:mixed = octet`) ; le mot reste de 64 bits. L'encodage en ligne
  actuel (`int` dans `n << 3`, `bool`) est récent et optimisé : les tags de
  largeur ne doivent pas forcer davantage de boxing sur `int`/`float` —
  mesurer `mixed` et `jsonb` avant/après.

### Builtins

- Les builtins numériques génériques gardent `int`/`float` (`int64`/`float64`).
  Les signatures fixes sont réservées aux opérations dont le contrat impose
  une largeur (octets, buffers, couleurs SDL). Affecter un résultat à un type
  plus étroit passe par une conversion vérifiée.

### Réalisation

1. **Types, opérations et règles** : les onze types fixes, `int`/`float`
   alias, dépassement contrôlé, conversions, `is numeric`, boucles à
   variable typée, exceptions, tags `mixed`, ABI des builtins.
2. **Représentations compactes** des champs d'objets et des collections
   typées fixes (`array<uint8>`, `array<float32>`, `map<string, uint8>`,
   `map<string, array<uint8>>`…) ; commencer par `array<uint8>` et
   `array<float32>`, où le gain est le plus net.

Mettre à jour les tests, la documentation canonique (`docs/EBNF.md`, y
compris le §31), les builtins concernés, `ocaracs`, l'extension VS Code et les
outils dépendant des types.

### Représentation et gains possibles

- Toute valeur occupe aujourd'hui un mot de 64 bits (registre, champ,
  élément, `mixed`). Seuls des layouts dédiés aux types fixes déclarés
  (`array<uint8>`, champs `uint8`) peuvent réduire le stockage ; `mixed`
  garde un mot tagué de 64 bits.
- **Hypothèse à examiner, pas une garantie** : un million d'éléments `uint8`
  occuperait environ 1 Mo de données brutes contre 8 Mo en mots de 64 bits,
  hors capacité inutilisée et métadonnées, avec un meilleur usage du cache.
  Pour les calculs scalaires, peu ou pas de gain : les registres restent de
  64 bits et les conversions peuvent ajouter des instructions. Mesurer les
  cas concrets avant de conclure.
- Cas d'usage : manipulation binaire (fichiers, réseau, images), SDL
  (couleurs `uint8`), données numériques, interopérabilité.

### Coût

- Langage : onze types fixes et leurs règles de conversion à apprendre ;
  `int`/`float` restent le choix par défaut sans avertissement.
- Compilateur : sema (conversions, plages, boucles typées), lowering
  (extension/troncature, opérations par largeur, contrôle de dépassement),
  codegen (types Cranelift par largeur, conventions d'appel).
- Runtime : boxing, conteneurs, (dé)sérialisation, affichage, `Convert`.
- Maintenance et tests : chaque builtin numérique et chaque conversion à
  couvrir pour chaque largeur.

## Priorité / Complexité

**Moyenne** (évolution du langage, aucun bug en jeu) — **Massive** : famille
complète, dépassement contrôlé, boucles typées, exceptions, ABI, builtins et représentations
compactes en deux phases.
