# Types numériques de largeur explicite (`int8`…`uint64`, `float16`…`float64`)

Statut : **conception validée le 2026-10-23**, avant implémentation. La famille
complète est retenue ; `int` et `float` seront des types numériques dynamiques,
distincts des types à largeur fixe. La réalisation est massive et sera menée
en deux phases, décrites ci-dessous.

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
| `int` | entier dynamique, largeur minimale selon la valeur | variable |

Flottants :

| Type | Plage finie approximative | Taille |
|---|---|---|
| `float16` | −65 504 à 65 504 | 2 octets |
| `float32` | −3,4 × 10³⁸ à 3,4 × 10³⁸ | 4 octets |
| `float64` | −1,8 × 10³⁰⁸ à 1,8 × 10³⁰⁸ | 8 octets |
| `float` | flottant dynamique, largeur minimale préservant exactement la valeur | variable |

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

### Types dynamiques et types fixes

- Ajouter les huit types entiers fixes (`int8`…`int64`, `uint8`…`uint64`) et
  les trois flottants fixes (`float16`…`float64`).
- `int` et `float` **ne sont pas des alias** : ce sont des types numériques
  dynamiques. Une variable `int` choisit la représentation entière minimale
  adaptée à sa valeur ; une variable `float` choisit la représentation flottante
  minimale qui préserve exactement sa valeur.
- Les variables restent toujours annotées (`var x:int = 5`) ; `var x = 5` n'est
  pas une forme valide.
- Dans `array<int>`, `map<string, int>` ou `map<string, array<int>>`, `int`
  reste le type numérique dynamique : le conteneur ne déduit pas une largeur
  fixe commune à partir des valeurs qu'il contient. Pour demander un stockage
  homogène compact, le type fixe est explicite, par exemple
  `array<uint8>`, `map<string, uint8>` ou `map<string, array<uint8>>`.
- Les expressions littérales numériques sont des `int`/`float` dynamiques et
  choisissent leur représentation minimale selon leur valeur. Dans une
  déclaration à type fixe, un littéral est vérifié directement dans le type
  déclaré.
- Un littéral dans une déclaration à largeur fixe est vérifié dans le type
  déclaré à la compilation. Une valeur constante hors plage est une erreur de
  compilation.

### Entiers, opérations et conversions

- Un entier dynamique non négatif utilise la plus petite largeur non signée
  qui le contient ; un entier négatif utilise la plus petite largeur signée.
  La largeur est recalculée après chaque opération :
  ```ocara
  var i:int = 255 // uint8
  i++             // uint16
  i -= 257        // int8
  ```
- Une opération à laquelle participe un `int` dynamique produit un `int`
  dynamique, même si l'autre opérande est un entier fixe. Par exemple :
  ```ocara
  var i:int = 5
  var b:uint8 = 251
  var x:int = i + b    // 256, représenté en uint16
  var y:uint8 = i + b  // IntegerException et NumericException
  ```
  Le résultat est calculé puis affecté ; le type cible ne change pas le type
  dynamique intermédiaire.
- Entre types entiers fixes, les opérations de même signe promeuvent vers la
  largeur supérieure ; une opération arithmétique signé/non signé est refusée.
  Les comparaisons entre largeurs et signes différents restent permises et
  exactes.
- Affecter un `int`/`float` dynamique à un type fixe effectue une conversion
  vérifiée selon le type cible. Pour un entier, la valeur doit tenir dans la
  plage ; pour un flottant, elle est arrondie à la précision IEEE cible si elle
  reste dans la plage finie. Un dépassement lève `IntegerException` ou
  `FloatException`. Entre deux types fixes, les règles d'élargissement sûr et
  de conversion explicite ci-dessous s'appliquent.
- Le dépassement de tout entier fixe, y compris `int64`, lève une exception.
  Un entier dynamique hors de la plage `int64`…`uint64` lève aussi une
  exception ; aucun de ces cas ne boucle.
- Les affectations entre types fixes n'autorisent implicitement que les
  élargissements qui préservent exactement toutes les valeurs source. Une
  conversion rétrécissante ou signé/non signé non sûre passe par `Convert`.
  Les littéraux explicitement typés peuvent être vérifiés directement dans
  leur contexte à la compilation.
- `Convert` conserve et étend sa matrice de conversions numériques, avec les
  méthodes d'instance correspondantes (`.toUint8()`, etc.). Une conversion
  numérique hors plage lève l'exception de la famille numérique, et non une
  troncature ou un bouclage.

### Flottants et comparaisons

- Les flottants fixes suivent IEEE 754 à leur précision déclarée. `float16`,
  `float32` et `float64` ne se convertissent implicitement vers une largeur
  supérieure que si toutes les valeurs source sont préservées exactement.
- `float` dynamique retient la plus petite largeur qui préserve exactement la
  valeur ; une conversion ou une opération qui dépasse la plage finie du type
  fixe lève `FloatException`. Les valeurs spéciales IEEE restent définies :
  NaN n'est égal à aucune valeur, les zéros signés sont égaux, et les infinis
  sont ordonnés.
- Les comparaisons entier/flottant et inter-largeurs sont exactes, sans
  conversion arrondie vers un flottant commun.
- Les clés flottantes de map sont canoniques : `+0.0` et `-0.0` désignent la
  même clé ; tous les NaN désignent une clé canonique.

### Exceptions et `mixed`

- `NumericException` est la classe de base ; `IntegerException` et
  `FloatException` en sont des sous-classes. Une erreur entière est donc
  interceptable par `IntegerException` ou `NumericException`, et une erreur
  flottante par `FloatException` ou `NumericException`.
- Les erreurs de dépassement entier, y compris les conversions hors plage,
  relèvent de `IntegerException`. Les dépassements flottants de calcul ou de
  conversion relèvent de `FloatException`. `MathException` reste notamment
  utilisée pour la division entière par zéro.
- `mixed` conserve le type et la largeur réellement stockés. Une valeur
  dynamique `int` stockée en `uint8` peut donc satisfaire à la fois `m is int`
  et `m is uint8`. Le mot `mixed` reste physiquement tagué sur 64 bits ; la
  largeur est exposée par ses tags, sans prétendre réduire la taille du mot.
- Le caractère dynamique de `int`/`float` est conservé dans les collections :
  `array<int>` et `map<string, int>` n'équivalent pas à `array<uint8>` et
  `map<string, uint8>`, même si toutes les valeurs présentes tiennent
  actuellement en 8 bits. La disposition compacte ciblée en phase 2 concerne
  les collections dont le type d'élément ou de valeur est explicitement fixe,
  y compris les types fixes imbriqués comme
  `map<string, array<uint8>>`.

### Builtins et réalisation

- Les builtins numériques génériques utilisent `int`/`float` dynamiques. Les
  signatures fixes restent réservées aux opérations dont le contrat impose
  une largeur (par exemple les octets et buffers). Les résultats numériques
  des builtins sont dynamiques ; l'affectation à un type fixe vérifie sa plage
  et lève l'exception numérique correspondante si nécessaire.
- La réalisation est découpée en deux phases :
  1. Types et opérations dynamiques/fixes, règles de sémantique, conversions,
     exceptions, tags `mixed` et ABI des builtins.
  2. Représentations compactes des champs d'objets et des collections
     explicitement typées avec des types numériques fixes (`array<uint8>`,
     `map<string, uint8>`, `map<string, array<uint8>>`, etc.) ; les collections
     de `int`/`float` dynamiques gardent leur sémantique dynamique et ne sont
     pas automatiquement converties en conteneurs homogènes à largeur fixe.
     `mixed` reste un mot tagué de 64 bits.
- Mettre à jour les tests, la documentation canonique (`docs/EBNF.md`, y
  compris le §31), les builtins concernés, `ocaracs`, l'extension VS Code et
  les outils dépendant des types. Mesurer les gains réels de mémoire et de
  performance ; ne pas les présumer.

### Représentation et gains possibles

- Aujourd'hui, toute valeur occupe un mot de 64 bits (registre, champ,
  élément, `mixed`). La largeur interne minimale de `int`/`float` ne signifie
  pas à elle seule qu'un `array<int>` ou `map<string, int>` est compact :
  seul un layout de collection dédié peut réduire son stockage. Les types fixes
  déclarés explicitement (`array<uint8>`, champs `uint8`, etc.) permettent de
  cibler sans ambiguïté des layouts compacts ; `mixed` garde un mot tagué de
  64 bits.
- **Hypothèse à examiner, pas une garantie** : des collections à type fixe
  (`array<uint8>` pour des octets, `array<float32>` pour des données numériques
  ou graphiques, ainsi que les types fixes imbriqués dans des maps) pourraient
  réduire la mémoire et améliorer l'usage du cache. À titre d'ordre de
  grandeur, un million d'éléments `uint8` occuperait environ 1 Mo de données
  brutes contre 8 Mo s'ils étaient stockés en mots de 64 bits, hors capacité
  inutilisée et métadonnées. Pour les calculs scalaires, les registres restent
  de 64 bits : peu ou pas de gain attendu, et des conversions peuvent même
  ajouter des instructions. Mesurer les cas concrets avant de conclure.
- Cas d'usage à identifier : manipulation binaire (fichiers, réseau,
  images), SDL (couleurs `uint8`), données numériques, interopérabilité.

### Coût

- Complexité du langage : neuf entiers et quatre flottants au lieu de deux
  types, avec des règles de conversion à apprendre.
- Compilateur : sema (règles de conversion et de plage), lowering
  (extension/troncature, opérations par largeur), codegen (types Cranelift
  par largeur, conventions d'appel).
- Runtime : boxing, conteneurs, (dé)sérialisation, affichage, `Convert`.
- Maintenance et tests : chaque builtin numérique et chaque conversion à
  couvrir pour chaque largeur.

## Priorité / Complexité

**Moyenne** (évolution du langage, aucun bug en jeu) — **Massive** : famille
complète, types dynamiques, exceptions, ABI, builtins et représentations
compactes en deux phases.
