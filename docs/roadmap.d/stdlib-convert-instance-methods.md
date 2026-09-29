# Méthodes d'instance manquantes pour `Convert` — `a.toInt()` au lieu de `Convert::strToInt(a)`

## Constat (vérifié directement)

Aucune des 19 méthodes statiques de `Convert` (`docs/builtins/Convert.md`)
n'a d'équivalent en méthode d'instance aujourd'hui :

```ocara
var a:string = "2"
var b:int = a.toInt()   // error: field 'toInt' not found in class 'String'
```

Le seul contournement actuel est la forme statique complète :
`Convert::strToInt(a)`.

**Le mécanisme de sucre statique→instance existant ne couvre pas ce cas.**
Il existe déjà (`allows_instance_sugar`, `src/sema/typecheck.rs:1376`) pour
7 classes (`String`, `Array`, `Map`, `JSON`, `HTTPRequest`, `HTTPResponse`,
`HTTPServerRequest`) — vérifié : `a.len()` fonctionne déjà et appelle
`String::len(a)` en coulisse. Mais ce mécanisme ne fait que permettre
d'appeler **la même méthode, sous le même nom**, en position d'instance —
`a.len()` marche parce qu'il appelle une méthode qui s'appelle DÉJÀ `len`.
Ce qui est demandé ici est différent : `a.toInt()` doit appeler
`Convert::strToInt`, un nom DIFFÉRENT (le préfixe de type source, redondant
une fois qu'on a un récepteur typé, est retiré). Ajouter simplement
`Convert` à `allows_instance_sugar` ne suffirait pas — ça permettrait
`a.strToInt()` (nom inchangé), pas `a.toInt()` (nom demandé). Il faut un
mécanisme de correspondance par méthode, pas juste une liste de classes
autorisées.

## Mapping demandé — les 19 méthodes de `Convert`

| Méthode statique actuelle | Type receveur | Méthode d'instance proposée |
|---|---|---|
| `strToInt(s)` | `string` | `s.toInt()` |
| `strToFloat(s)` | `string` | `s.toFloat()` |
| `strToBool(s)` | `string` | `s.toBool()` |
| `strToArray(s, sep)` | `string` | `s.toArray(sep)` |
| `strToMap(s, sep, kv)` | `string` | `s.toMap(sep, kv)` |
| `intToStr(n)` | `int` | `n.toStr()` |
| `intToFloat(n)` | `int` | `n.toFloat()` |
| `intToBool(n)` | `int` | `n.toBool()` |
| `floatToStr(f)` | `float` | `f.toStr()` |
| `floatToInt(f)` | `float` | `f.toInt()` |
| `floatToBool(f)` | `float` | `f.toBool()` |
| `boolToStr(b)` | `bool` | `b.toStr()` |
| `boolToInt(b)` | `bool` | `b.toInt()` |
| `boolToFloat(b)` | `bool` | `b.toFloat()` |
| `arrayToStr(arr, sep)` | `array<T>` | `arr.toStr(sep)` |
| `arrayToMap(arr, kv)` | `array<T>` | `arr.toMap(kv)` |
| `mapToStr(m, sep, kv)` | `map<K,V>` | `m.toStr(sep, kv)` |
| `mapKeysToArray(m)` | `map<K,V>` | `m.keysToArray()` |
| `mapValuesToArray(m)` | `map<K,V>` | `m.valuesToArray()` |

Convention utilisée dans ce tableau : retirer le préfixe de type SOURCE
(redondant, porté par le récepteur), garder `to<Cible>` — cohérent avec
l'exemple donné par l'utilisateur (`strToInt` → `toInt`). Les deux méthodes
`map*ToArray` n'ont pas de préfixe de type source à retirer (leur nom décrit
déjà QUELLE partie de la map extraire, pas juste une conversion de type
brute) — nommage proposé, pas encore tranché explicitement par l'utilisateur.

## Ce qu'il faut trancher avant d'implémenter

- **Le tableau ci-dessus est une proposition, pas une décision actée** —
  en particulier `keysToArray`/`valuesToArray` (vs. un nom plus court comme
  `.keys()`/`.values()`, déjà un vocabulaire courant pour les maps dans
  d'autres langages).
- **Mécanisme d'implémentation** : `allows_instance_sugar` (liste de
  classes) ne suffit pas — il faut une table de correspondance
  `(type receveur, nom d'instance) → nom de méthode statique réelle`,
  vérifiée au même endroit (`src/sema/typecheck.rs`, résolution d'un appel
  `Expr::Field`/`Expr::Call` sur un récepteur de type primitif/`array`/`map`)
  mais avec une logique de lookup différente de celle qui existe déjà pour
  les 7 classes actuelles.
- **`int`/`float`/`bool`/`array`/`map` n'ont aujourd'hui AUCUNE méthode
  d'instance** (contrairement à `string`, qui en a déjà via `String`) —
  ajouter des méthodes d'instance à des types primitifs qui n'en ont jamais
  eu est un changement plus large que d'étendre une classe qui en a déjà
  (`String`) : vérifier qu'aucune limitation actuelle du compilateur
  n'empêche spécifiquement `int`/`float`/`bool` d'avoir des méthodes
  d'instance (repère utile : la classe `Array`/`Map` ont déjà des méthodes
  d'instance aujourd'hui pour leurs propres méthodes, `String` aussi — donc
  le mécanisme des méthodes d'instance sur un type non-classe-utilisateur
  existe déjà en général, juste jamais exercé encore pour `int`/`float`/
  `bool` spécifiquement).
- **Nom de la classe `Convert`** : question posée par l'utilisateur,
  réponse déjà donnée en discussion — garder `Convert`, ne pas renommer en
  `Cast` (qui évoquerait une réinterprétation directe sans logique de
  parsing, alors que `Convert::strToInt("abc") → 0` est une vraie décision
  de repli, pas une réinterprétation binaire). Ce ticket ne touche donc pas
  au nom de la classe, seulement à l'ajout de méthodes d'instance.

## Priorité / Complexité

**Priorité Haute**, demandé explicitement à ce niveau. Complexité non
évaluée précisément avant investigation — probablement Structurel : pas
seulement un ajout de sucre syntaxique pour des classes déjà pourvues de
méthodes d'instance (`String`/`Array`/`Map`), mais l'introduction de
méthodes d'instance sur des types (`int`/`float`/`bool`) qui n'en ont jamais
eu, plus un mécanisme de correspondance par nom (pas juste par classe)
jamais nécessaire jusqu'ici.

## Fichiers clés

`src/sema/typecheck.rs` (`allows_instance_sugar` et sa logique de résolution
d'appel, à étendre ou compléter par un mécanisme de correspondance par nom),
`src/builtins/convert.rs` (signatures actuelles de `Convert`),
`docs/builtins/Convert.md`, `docs/EBNF.md` (si les types primitifs gagnent
la capacité générale d'avoir des méthodes d'instance, documenter cette
règle explicitement si elle ne l'est pas déjà).
