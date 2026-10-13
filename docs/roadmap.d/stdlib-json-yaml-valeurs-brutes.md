# JSON / YAML : valeurs brutes mal interprétées (SIGSEGV, `1` lu `true`, flottants perdus)

Statut : **à corriger** — constaté le 2026-10-13 (audit de la roadmap).

## Constat

### 1. Tableau concret dans un conteneur `mixed` — SIGSEGV ou valeurs fausses

```ocara
var flat:array<int> = [123456, 5]
var mm:array<mixed> = [flat]
JSON::encode(mm)               // SIGSEGV (YAML::encode aussi)
JSON::encode([flat, flat])     // SIGSEGV
var fl:array<float> = [0.0, 1.5]
JSON::encode([fl])             // [[null,4609434218613702656]]
// {"k": flat} dans une map<string,mixed> → {"k":[4609434218613702656,null]}
```

`value_to_json` (`runtime/src/lib.rs`, ~l.3388) ignore le drapeau `FLAG_RAW`
(`runtime/src/rc.rs`) que porte pourtant le tableau, et lit chaque élément brut
comme une valeur `mixed`. Le drapeau dit « brut » sans dire si c'est un `int`,
un `float` ou un `bool`.

### 2. Entier `1` confondu avec `true`, flottants perdus au décodage

```ocara
JSON::encode([1, 2, 0, true])     // [true,2,0,true,false]
YAML::encode({"x": 1})            // x: true
JSON::decode("{\"p\":3.75,\"q\":1,\"r\":true}")   // p=null, r relu 1
var one:mixed = 1
one is bool                       // true ; un match envoie 1 dans « is bool »
```

- `runtime/src/lib.rs` ~l.3448 : heuristique `val == 1 → Bool` à l'encodage ;
- ~l.3510-3519 : au décodage, un booléen n'est pas boxé et un flottant devient `0` ;
- `runtime/src/typecheck.rs` ~l.96 : `__is_bool` vrai pour `0` et `1`.

Les booléens rangés dans un `mixed` sont désormais toujours boxés : ces
heuristiques ne se justifient plus. Signalé comme « limitation » dans
`docs/builtins/JSON.md` (~l.299) et `docs/EBNF.md` (~l.885), sans suivi.

## Correction attendue

1. Faire porter au tableau/à la map la forme de ses éléments bruts (int/float/bool)
   ou les boxer au moment de les placer dans un `mixed` ; `value_to_json` et
   l'encodeur YAML lisent cette forme.
2. Boxer booléens et flottants dans `json_to_value`, retirer l'heuristique
   `1 → true` et le cas `0/1` de `__is_bool`.
3. Mettre à jour `JSON.md`, `YAML.md` et l'EBNF (retirer la « limitation »).

## Validation

Tests `examples/tests/` : aller-retour encode/decode sur `array<int>`,
`array<float>`, `array<bool>` imbriqués dans `mixed`, valeurs `0`, `1`, `true`,
`3.75`, grand entier ; `is bool`/`is int` sur `1` ; `make regression`.

## Priorité / Complexité

**Haute** (crash et corruption silencieuse des données sérialisées) —
**Structurel** (représentation des éléments bruts vue par le runtime).
