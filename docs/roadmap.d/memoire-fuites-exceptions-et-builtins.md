# Fuites mémoire : exceptions, méthodes d'instance JSON, HTMLComponent

Audit du 2026-10-19. Reproduit sur le binaire courant (`--leak-check` et pic
mémoire à deux tailles, voir `docs/benchmarking.md`).

## Exceptions

1. **`raise` dans un handler `on` : l'exception rattrapée `e` fuit.**
   ```ocara
   try {
       try { raise use MyErr("first") } on e is MyErr { raise "second" }
   } on e2 { t = t + 1 }
   ```
   100 itérations → `100 × MyErr (allouées 100, libérées 0)` ; 3,6 Mo à
   20 000, 12,1 Mo à 200 000. Aussi `raise e`, chaîne relevée, à travers un
   appel. `e` est déclarée `declare_local(…, false)` sans enregistrement pour
   le déroulement (`exceptions.rs` ~l.395-405).
2. **`return` dans un handler `on` : les locales de la fonction fuient.**
   `var xs:array<string> = ["a" + "b"]` puis `try { raise "x" } on e { return
   3 }` → `1 × array`, `1 × string`. Même chose pour `result 3` dans un `on`
   d'un bloc runtime (`exceptions.rs` ~l.599-642 : pas de destructions de
   sortie anticipée).
3. **Constructeur qui lève : l'objet fuit** (`class Bad { init(v:string) {
   self.v = v  raise use MyErr("ctor", 3) } }` → `1 × Bad`).
4. **Cellule de capture** : nom de variable de boucle réutilisé + `raise` au
   milieu d'une boucle + capture → `1 × variable capturée` (sans la première
   boucle, rien ne fuit).

## Builtins

5. **Méthodes d'instance JSON : le résultat fuit** (`JSON::encode(m)` est
   propre) :

   | Appel | Pic 20 000 → 200 000 |
   |---|---|
   | `map.encode()`, `array.encode()`, `[1,2,3].encode()` | ~4,9 → 13,4 Mo |
   | `s.pretty()`, `s.minimize()` | ~4,1 → 12,5 Mo |
   | `s.decode()` | 7,7 → 49,8 Mo |

   Aussi dans un gabarit `${user.encode()}`. Le cas `is_json_method`
   (`src/lower/expr.d/lower.rs` ~l.509-541) émet un `Inst::Call` qui
   contourne l'enregistrement de la valeur possédée.
   `examples/builtins/json.oc` laisse 14 valeurs.
6. **`HTMLComponent`** : `register` retient la closure sans la garder ;
   ré-enregistrer ou `unregister` ne la relâche jamais (9,4 → 62 Mo,
   20 000 → 200 000). `use HTMLComponent("…")` fait un `Box::into_raw` jamais
   détruit (invisible au bilan : 96 Mo pour 10⁶ instances).
   `examples/builtins/html.oc` laisse 5 closures
   (`runtime/src/htmlcomponent.rs` ~l.109-128).

Voir aussi : tâches `async` jamais résolues
([async-resolvable-handles.md](async-resolvable-handles.md)), ressource native
de `main` fermée dans `exit`
([langage-runtime-blocs-variables.md](langage-runtime-blocs-variables.md)),
fonction capturée rendant une `string`
([langage-closure-metadonnees-capture.md](langage-closure-metadonnees-capture.md)),
`${r.body()}` ([builtins-types-retour-manquants.md](builtins-types-retour-manquants.md)).

## Outil

`grep -n "Box::into_raw\|Box::from_raw" runtime/src/*.rs` liste les builtins
dont l'instance Rust n'a pas de destruction : à vérifier un par un (fuites
invisibles à `--leak-check`).

## Priorité / Complexité

**Haute** (fuites) — **Légère**.

## Fichiers clés

`src/lower/stmt.d/statements.d/exceptions.rs`, `src/lower/expr.d/lower.rs`,
`runtime/src/htmlcomponent.rs`, `src/lower/builder.d/classes.rs` (constructeur).
