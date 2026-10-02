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

## Décisions et mise en œuvre

- **Tableau ci-dessus retenu**, sauf `keysToArray`/`valuesToArray` : pas
  d'alias, `m.keys()`/`m.values()` (classe `Map`) font déjà exactement la
  même chose (`Convert_mapKeysToArray` appelle `Map_keys`) — 17 méthodes.
- **Mécanisme** : `src/sema/convert_sugar.rs` — table
  `(type receveur, nom d'instance) → méthode Convert`, consultée par la sema
  (seule à connaître le vrai type : `string`/`array`/`map` sont tous des
  pointeurs au niveau IR) au début de la résolution d'un appel de méthode.
  L'appel est réécrit en `Convert::<méthode>(receveur, args...)`
  (`AstRewrites::calls`, appliqué par `core::named_args::rewrite_program`,
  qui ajoute aussi l'import `ocara.Convert` s'il manque) — le lowering ne
  voit qu'un appel statique ordinaire. Arguments nommés supportés (noms de
  la méthode `Convert`, receveur exclu).
- E37 (`MethodCallOnNonClass`) liste désormais les conversions disponibles
  sur `int`/`float`/`bool` au lieu d'affirmer « which has no methods ».

Corrigés au passage (préexistants, reproduits en forme statique) :
- appel de méthode chaîné sur le résultat d'un appel statique BUILTIN
  (`Convert::arrayToMap(arr, "=").size()` → `null`) : `method_ret_class`
  couvre maintenant aussi les méthodes builtin ;
- indexation directe du résultat d'un appel (`Convert::strToMap(...)["k"]`,
  `getConfig()["k"]` → `null`, lu comme un tableau) : `is_map_target`
  reconnaît un appel dont la classe de retour est `Map`.

**ocaraunit masquait des échecs** (découvert en écrivant le test 69) : un
`assert*` en échec lève une exception qui termine le binaire, et ocaraunit
ne signalait un échec d'exécution que si AUCUNE assertion n'avait réussi
avant — tout échec en cours de fichier disparaissait derrière « 0 FAIL »,
avec le reste du fichier. De plus, son cache n'était indexé que sur le
contenu du fichier de test : un changement du compilateur, du runtime ou
d'un fichier importé réutilisait l'ancien binaire. Les deux sont corrigés
(`tools/ocaraunit/src/main.rs`), et le bandeau d'exception non rattrapée du
runtime affiche désormais le message de l'exception. Cela a révélé trois
bugs préexistants, ajoutés à la roadmap :
[memoire-nested-array-zero-json](memoire-nested-array-zero-json.md),
[langage-negative-class-const](langage-negative-class-const.md),
[langage-variadic-bool-not](langage-variadic-bool-not.md).

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
