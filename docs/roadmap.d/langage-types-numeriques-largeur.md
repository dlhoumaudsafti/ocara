# Types numériques de largeur explicite (`int8`…`uint64`, `float16`…`float64`)

Statut : **réflexion non tranchée** — ouverte le 2026-10-18. Aucune décision
n'est prise : cette fiche décrit l'existant (vérifié) et les questions à
trancher avant toute implémentation.

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
| `int` | alias envisagé de `int64` | 8 octets |

Flottants :

| Type | Plage finie approximative | Taille |
|---|---|---|
| `float16` | −65 504 à 65 504 | 2 octets |
| `float32` | −3,4 × 10³⁸ à 3,4 × 10³⁸ | 4 octets |
| `float64` | −1,8 × 10³⁰⁸ à 1,8 × 10³⁰⁸ | 8 octets |
| `float` | alias envisagé de `float64` | 8 octets |

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
- **Division entière** : `sdiv`/`srem` sans garde — `x / 0` arrête le
  programme (SIGILL), `x % 0` et `INT_MIN / -1` aussi (SIGFPE), sans
  exception Ocara. Défaut suivi à part en Priorité Haute (voir
  [langage-division-entiere-par-zero](langage-division-entiere-par-zero.md)).
  Une constante de classe calculée à la compilation rejette déjà la division
  par zéro et le débordement (`docs/diagnostics.md`, E55).
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

## Questions à trancher

### Dépassement, conversion, affectation

- Dépassement d'une opération : bouclage (comme aujourd'hui pour `int`),
  exception, saturation, ou au choix (opérateurs dédiés) ? Le même choix
  pour toutes les largeurs ?
- Littéral hors plage du type cible (`var b:uint8 = 300`) : erreur de
  compilation, comme pour `int` aujourd'hui ?
- Affectation entre largeurs : élargissement implicite autorisé
  (`int16` → `int32`) et rétrécissement explicite seulement ? Ou aucune
  conversion implicite, dans la ligne actuelle (`int` ↔ `float` refusé) ?
- Signé ↔ non signé : conversion explicite obligatoire ? Que vaut
  `uint8(-1)` — erreur, bouclage (255), exception ?
- Type d'un littéral non annoté (`var x = 5`) : toujours `int` (= `int64`) ?
- Opérations mixtes entre largeurs (`int8 + int32`) : erreur, ou promotion
  vers la plus large ? Division d'entiers non signés, décalages, modulo
  négatif.

### Flottants

- Précision : `float32` (~7 chiffres décimaux significatifs) et `float16`
  (~3 chiffres) perdent de l'information dès la conversion depuis `float64` ;
  conversion implicite interdite, ou permise avec avertissement ?
- Valeurs spéciales (`inf`, `-inf`, `NaN`, `-0.0`, nombres sous-normaux) :
  comportement dans `equal`, dans le tri, comme clé de map (aujourd'hui
  convertie en texte), à l'affichage.
- `float16` : support réel de la cible (Cranelift, x86-64, aarch64, Android)
  et de Rust côté runtime à vérifier ; à défaut, conversion vers `float32`
  pour chaque calcul — coût et intérêt à évaluer.
- Comparaison `int`/`float` : la règle d'élargissement actuelle s'étend-elle
  à toutes les paires ? `uint64` contre `float64` n'est pas représentable
  sans perte au-delà de 2⁵³.

### Compatibilité

- Signatures : surcharge par largeur ou non ; passage d'un `int8` à un
  paramètre `int` (élargissement implicite ?).
- Collections : `array<uint8>` stocké en `Vec<i64>` (aucun gain mémoire) ou
  conteneur spécialisé (`Vec<u8>`) — avec des conséquences sur tout le
  runtime des conteneurs, le drapeau « éléments bruts », JSON/YAML,
  `Map::forEach`, la collecte des cycles.
- Propriétés : champ de largeur réduite placé dans un mot de 8 octets
  (aucun gain) ou disposition compacte (alignement, masques de classe du
  comptage de références, `field_offset`).
- Constantes et `enum` (aujourd'hui `int`) ; `const` calculées à la
  compilation (E55) : plage du type à vérifier.
- `mixed` : boxing à définir pour chaque largeur ; les tests de type
  (`is int`, `__is_int`) doivent-ils distinguer `int8` de `int64` ? Il
  faudrait alors une étiquette de largeur dans la valeur boxée.
- Builtins : quels builtins acceptent ou rendent des largeurs réduites
  (`Math`, `Convert`, `Array`, SQLite `INTEGER`/`REAL`, JSON, `File` binaire,
  HTTP) ; ABI `i64`/`f64` actuelle à étendre ou conversions aux frontières.
- FFI future : si une FFI vers des bibliothèques C est un jour exposée, des
  types de largeur fixe seront nécessaires pour décrire leurs signatures —
  argument à peser, mais pas un besoin actuel.
- Outillage : EBNF §6.1 et §31, `ocaracs`, extension VS Code (coloration,
  complétion), serveur de langage, `docs/builtins/*.md`.

### Représentation et gains possibles

- Aujourd'hui, toute valeur occupe un mot de 64 bits (registre, champ,
  élément, `mixed`). Une largeur plus faible ne réduirait la mémoire que si
  la représentation change aussi : conteneurs spécialisés, champs compactés,
  tampons binaires. Sans cela, elle n'apporterait que la vérification de
  plage.
- **Hypothèse à examiner, pas une garantie** : des tableaux compacts
  (`array<uint8>` pour des octets, `array<float32>` pour des données
  numériques ou graphiques) pourraient réduire la mémoire et améliorer
  l'usage du cache. Pour les variables scalaires, les registres restent de
  64 bits : peu ou pas de gain attendu, et des conversions (extension,
  troncature) peuvent même ajouter des instructions. À mesurer sur des cas
  concrets avant de conclure.
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

## Pistes possibles (non décidées)

- Ne rien ajouter et documenter `int`/`float` comme seuls types numériques.
- Ajouter seulement des **types de stockage** (`array<uint8>` compact, champs
  compacts), les calculs restant en `int`/`float`.
- Ajouter la famille complète, avec `int`/`float` comme alias de
  `int64`/`float64`.

## Priorité / Complexité

**Moyenne** (évolution du langage, aucun bug en jeu) — **Massive** si la
famille complète est retenue, à réévaluer selon la piste choisie.
