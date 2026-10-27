# `x["clé"]` et `x.clé` sur une valeur `mixed` contenant une map : valeur fausse

Statut : **bug confirmé** — ouvert le 2026-10-26 (constaté pendant
la notation pointée sur les maps, `docs/EBNF.md` §7.4), priorité haute :
valeur fausse silencieuse.

## Constat (vérifié le 2026-10-26)

```ocara
var j:mixed = JSON::decode("{\"user\": \"bob\"}")
IO::writeln(j["user"])          // affiche "null" au lieu de "bob"

var mm:map<string, mixed> = {"a": 1}
var x:mixed = mm
IO::writeln(x["a"])             // "null" au lieu de 1
```

Compile sans erreur ni avertissement (hors W02 sur la variable `mixed`).

Même famille : `j.user` sur ce `mixed` compile et lit le début du bloc de la
map (affiche `4`) — la notation pointée `truc.A` (`docs/EBNF.md` §7.4)
n'est résolue que sur un type statique map ; sur `mixed`, `.nom` reste un
accès de champ. Un refus à la compilation de tout `.nom` sur `mixed` a été
essayé puis retiré : il casse le handler catch-all `on e { e.message }`
(`e` est un `mixed`, `raise "texte"` étant permis) —
`src/sema/tests/exception_binding.rs`, `catch_all_binding_stays_permissive`.

## Cause

`lower_expr`, branche `Expr::Index` (`src/lower/expr.d/lower.rs`) choisit
`__map_get` ou `__array_get` par `is_map_target`
(`src/lower/expr.d/helpers.rs`), qui ne connaît que les types statiques
(variables map, champs map, appels à retour `Map`, index chaînés). Un
receveur `mixed` tombe dans `_ => false` : `__array_get(map, "user")` lit le
bloc de la map comme un tableau, avec l'adresse de la chaîne comme indice.
L'écriture (`x["a"] = 1`, `lower_assign`) passe par la même décision.

La documentation (`docs/builtins/JSON.md`) contourne en typant d'abord
(`var m:map<string, mixed> = data`), ce qui fonctionne.

## Pistes

1. **Résolution à l'exécution, limitée aux receveurs `mixed`** :
   `__mixed_index(val, key)` lit l'étiquette du bloc (`TAG_MAP` /
   `TAG_ARRAY`, `runtime/src/typecheck.rs`) et délègue à `__map_get` ou
   `__array_get` ; une autre valeur lève une `Exception`. Le lowering ne
   connaît pas le type sema : il faut une réécriture décidée par la sema
   (comme `AstRewrites::map_dots`) ou un appel `Convert` explicite.
   Aucun coût sur les receveurs de type connu.
   Même fonction pour `x.clé` : clé si la valeur est une map, champ sinon
   (le champ d'un objet n'est pas connu par son nom à l'exécution — à
   étudier, ou limiter aux exceptions via `message`/`code`/`source`).
2. **Refus à la compilation** : diagnostic proposant de typer la valeur.
   Casse le code existant qui indexe un `mixed`, et le catch-all
   `on e { e.message }` pour `x.clé` — à mesurer sur `examples/`.

La piste 1 garde l'usage naturel des valeurs JSON. À trancher.

## Critères d'acceptation

- Les deux repros affichent `bob` et `1` (piste 1) ou sont refusés avec
  un diagnostic qui propose la forme typée (piste 2).
- Écriture indexée couverte de la même façon.
- `make perf` sans régression sur `map`, `mapint`, `jsonb`, `mixed`.

## Complexité

Légère (piste 2) à Structurel (piste 1 : réécriture sema + fonction runtime).
