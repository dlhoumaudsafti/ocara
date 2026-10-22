# Accès aux maps par notation pointée (`truc.A` pour `truc["A"]`)

Statut : **à étudier puis implémenter** — ouverte le 2026-10-22, priorité
moyenne. Rien n'est implémenté ; cette fiche décrit l'existant (vérifié), le
comportement attendu, les questions à trancher et les critères
d'acceptation.

## Proposition

Écrire `truc.A` pour lire ou écrire la clé chaîne `"A"` d'une map, quand la
clé est un identifiant valide :

```regex
^[a-zA-Z_]([a-zA-Z0-9_]+)?$
```

(équivalent à `^[a-zA-Z_][a-zA-Z0-9_]*$` : lettre ASCII ou `_`, puis lettres
ASCII, chiffres ou `_`).

```ocara
var truc:map<string, int> = {
    "A": 1,
    "B": 2
}

truc["A"]       // 1 — inchangé
truc.A          // 1 — raccourci de truc["A"]
truc.A = 3      // raccourci de truc["A"] = 3
```

C'est un **raccourci**, pas un remplacement : `truc["A"]` reste disponible,
et seul possible pour une clé dynamique (`truc[k]`) ou une clé qui n'est pas
un identifiant valide (`truc["foo-bar"]`, `truc["1a"]`, `truc[""]`).

## État actuel (vérifié le 2026-10-22)

- **Lecture refusée** : `truc.A` donne
  `error: field 'A' not found in class 'Map'`
  (`src/sema/typecheck.rs`, accès `Expr::Field`).
- **Écriture refusée** de la même façon depuis le 2026-10-22 : elle
  compilait et écrivait à l'offset 0 du bloc de la map
  ([sema-affectation-champ-inexistant](sema-affectation-champ-inexistant.md),
  `src/sema/unknown_field.rs`). La notation pointée lèvera ce refus pour
  une `map<string, V>`.
- **`mixed` permissif** : `j.user` sur un `mixed` (ex. résultat de
  `JSON::decode`) compile et lit l'offset 0 du bloc — un `mixed` contenant
  une map affiche `4` au lieu de sa clé `"user"`
  ([sema-acces-champ-index-non-classe](sema-acces-champ-index-non-classe.md) :
  « `mixed` reste permissif »).
- **Accès par index** (`docs/EBNF.md` §7.4) : `IndexAccess ::= Expression
  "[" Expression "]"`. Une clé absente rend la valeur nulle du type (`0`,
  `null`) sans exception ; `Map::get(m, clé)` lève une `MapException`
  (`docs/builtins/Map.md`).
- **Type par clé** (`docs/EBNF.md` §7.3, `src/sema/map_keys.rs`) : une map
  initialisée par un littéral à clés chaîne garde le type de chaque clé
  (`details['car']` est un `CarEntity`, `details['rows_html']` une `string`).
- **Identifiant** (`docs/EBNF.md` §8) : `Identifier` accepte les lettres
  Unicode (`é`, `ß`), plus large que la règle ASCII ci-dessus ; les
  mots-clés réservés en sont exclus.
- **Méthodes** : une map s'utilise aussi par méthodes (`m.size()`,
  `m.has(k)`…) ou fonctions (`Map::size(m)`, `Map::keys(m)`…). `m.size`
  sans parenthèses est refusé en lecture.
- **Littéral** : `{A: 1}` lit `A` comme une **expression** (variable), pas
  comme la clé `"A"` (`MapEntry ::= Expression ":" Expression`).
- **Chemin d'exécution de `truc["A"]`** : la clé est un littéral rangé en
  `.rodata` (aucune chaîne allouée ni convertie), passée à `__map_get` →
  `with_key_text` → `MapTable::get` : hachage du texte à chaque accès
  (`runtime/src/map_hash.rs`, graine aléatoire **propre à chaque map**, donc
  non calculable à la compilation), sondage, puis comparaison de la clé — par
  pointeur d'abord, si la clé a été rangée depuis le même littéral
  (`runtime/src/map_table.rs`, `is_key`).

## Comportement attendu

- `truc.A` ≡ `truc["A"]` pour une map dont le type statique est
  `map<string, V>` : même valeur, même clé absente (valeur nulle, pas
  d'exception), même type par clé, même type `V` sinon.
- Écriture, affectation composée et incrément : `truc.A = v`,
  `truc.A += v`, `truc.A++` — comme leurs équivalents indexés (une clé
  absente est créée).
- Chaînage : `cfg.db.host` sur `map<string, map<string, string>>`,
  `self.options.debug` sur un champ de type map, mélange `cfg.db["port"]`.
- Clé non valide : `truc["foo-bar"]` seulement ; `truc.foo-bar` reste
  `truc.foo - bar` (soustraction), avec une erreur si `foo` ou `bar`
  n'existe pas. `truc.1a` reste une erreur de syntaxe.
- Clé dynamique : `truc[k]` inchangé.
- Aucun changement pour les objets : `obj.champ` reste un champ quand le
  type statique est une classe.
- Résolution **à la compilation**, sur le type statique : aucun test ajouté
  à l'exécution sur le chemin de `truc["A"]` ou de `obj.champ`.

## Cas à vérifier

- Lecture, écriture, `+=`, `++`, clé absente, clé créée par affectation.
- Map à clés `int`/`float`/`bool` (`map<int, V>`) : `m.A` refusé
  (diagnostic clair) ou autre règle, selon la décision.
- Clés non identifiants : `"foo-bar"`, `"1a"`, `""`, `"a b"`, `"é"` —
  seulement par `[]`.
- Clé portant le nom d'une méthode : `truc.size`, `truc.keys` (sans
  parenthèses) face à `truc.size()`.
- Clé mot-clé : `truc.class`, `truc.if`, `truc.match`.
- Chaînage de maps imbriquées, map champ d'objet (`self.m.A`), map
  élément de tableau (`maps[0].A`).
- `map<string, V>|null` valant `null` ; `mixed` contenant une map, un
  objet, une chaîne, un entier.
- Type par clé : `details.car` est un `CarEntity`, `details.rows_html = car`
  est refusé comme `details['rows_html'] = car`.
- Clés dynamiques et invalides toujours correctes après le changement
  (régression).
- Fermetures et générateurs (map capturée), `try`, `for k has v in truc`
  avec `truc.A` dans le corps (voir le parcours en place,
  `src/lower/stmt.d/statements.d/map_loop_scan.rs` : une écriture
  `truc.A = …` doit compter comme une écriture indexée).
- Fuites (`make leak-check`) et sortie de `--leak-check` sur les nouveaux
  exemples.

## Questions de conception à trancher

1. **Collision avec les méthodes** : `truc.size` (clé `"size"`) et
   `truc.size()` (méthode) peuvent coexister — appel = méthode, sans appel =
   clé ? Ou refuser la notation pointée pour un nom de méthode de `Map`
   (avec renvoi vers `truc["size"]`) ? Effet d'un futur ajout de méthode sur
   du code existant.
2. **Mots-clés** : `truc.class` — accepté après un point (le lexer rend un
   mot-clé), ou réservé ?
3. **Unicode** : la règle demandée est ASCII alors que `Identifier` accepte
   `été` — refuser `truc.été` (diagnostic orientant vers `truc["été"]`) ou
   s'aligner sur `Identifier` ?
4. **Types de clé** : réservé à `map<string, V>` ? `map<mixed, V>` ?
   `map<int, V>` refusé ?
5. **`mixed`** : `j.user` sur un `mixed` — résolution dynamique (clé si la
   valeur est une map, champ si c'est un objet, exception sinon), avec le
   coût d'un test à l'exécution limité aux accès sur `mixed` ? Ou exiger un
   type statique map (`(j as map<string, mixed>).user`, à voir) ? Dans tous
   les cas, la lecture actuelle de l'offset 0 n'est pas acceptable pour une
   map.
6. **`null`** : receveur `map|null` nul — même règle qu'un objet nul
   (`Exception` « Cannot access 'A' on null », `__null_receiver`) ou valeur
   nulle comme une clé absente ?
7. **Où désucrer** : le parseur ne connaît pas les types ; la sema décide
   (type statique map) et le lowering réécrit en `Expr::Index` avec clé
   littérale (`is_map_target`, `src/lower/expr.d/helpers.rs`), ou un nouveau
   nœud ? Effet sur `Stmt::Assign`, `IncDec`, affectations composées et le
   type par clé.
8. **Outils** : coloration et complétion des clés connues (type par clé) dans
   l'extension VS Code (`tools/`) et le LSP (`src/lsp/`) ; `ocaracs`.

## Performances

Exigence : **aucune régression** de `truc["A"]`, `truc[k]` ni des autres
accès aux maps. La notation pointée doit être résolue à la compilation
(question 7) pour ne rien ajouter au chemin indexé.

À étudier, sans présumer que `truc.A` sera plus rapide par lui-même :
aujourd'hui `truc["A"]` n'alloue ni ne convertit déjà aucune chaîne ;
le coût est le hachage à chaque accès, le sondage et la comparaison. Pistes,
valables aussi pour `truc["A"]` avec une clé littérale :

- **Cache par site d'appel** : retenir le rang de la paire trouvée au
  dernier accès et vérifier d'abord cette paire (pointeur de clé égal), comme
  `__map_value_at` pour `for k has v in m` ;
- **Hachage précalculé** : impossible tel quel (graine par map) ; à évaluer
  avec une graine par processus pour les clés littérales, sans affaiblir la
  résistance aux clés hostiles venues du réseau ;
- rien à gagner sur `truc[k]` (clé dynamique).

Si l'une de ces pistes est retenue, l'appliquer aussi à `truc["A"]` : la
notation pointée n'en est que le raccourci.

### Mesures avant/après

- Relevé **avant** toute modification : `make perf` complet et
  `make perf-extra` (mémoire), en particulier `map`, `mapint`, `mapiter`,
  `jsonb`, `sqlite`, `yamldec`.
- Nouvelle mesure `ci/perf/mapconst.*` (Ocara, Python, PHP, Perl, C, C++,
  Rust, entrée dans `ci/perf/benches.txt`) : lectures et écritures répétées
  de clés constantes, dont des clés lues dans une boucle chaude.
- Comparaison A/B dans Ocara : même programme en `truc["A"]` et en `truc.A`
  (temps et instructions générées, `ocara --dump`).
- Relevé **après** : mêmes commandes, même machine, au même moment
  (minimum de 5 exécutions, voir `docs/benchmarking.md`) ; chiffres
  consignés ici.

## Critères d'acceptation

- `truc.A` lit, écrit (`=`, `+=`, `++`) et chaîne comme `truc["A"]` pour une
  clé conforme à `^[a-zA-Z_]([a-zA-Z0-9_]+)?$`, avec le même typage (type par
  clé compris) et la même sémantique de clé absente.
- `truc["A"]`, `truc[k]` et `truc["foo-bar"]` inchangés ; toute clé non
  identifiant reste accessible par `[]`.
- Questions 1 à 7 tranchées et documentées ; tout usage refusé produit un
  diagnostic qui propose la forme `truc["…"]`.
- Plus aucune lecture de l'offset 0 d'une map via un `mixed` si la
  question 5 retient la résolution dynamique.
- Aucune régression de performance : `make perf` et `make perf-extra` sans
  nouvel échec, `map`/`mapint`/`mapiter`/`jsonb` au moins aussi rapides
  qu'avant ; mesure `mapconst` ajoutée ; résultat de l'étude d'accélération
  consigné (retenue ou écartée, chiffres à l'appui).
- Tests : sema (`src/sema/tests/`), lowering (`src/lower/`), exemple
  ocaraunit dans `examples/tests/`, `make tests`, `make tests-runtime`,
  `make regression`, `make leak-check` verts.
- Documentation : `docs/EBNF.md` (§7.3, §7.4, §16.3 et §31),
  `docs/builtins/Map.md`, diagnostics nouveaux dans `docs/diagnostics.md`,
  extension VS Code et LSP si la coloration ou la complétion change.

## Complexité

**Légère** si la notation est limitée aux maps de type statique connu
(désucrage sema/lowering) ; **Structurel** si `mixed` reçoit une résolution
dynamique (question 5).

## Fichiers clés

`src/parsing/parser.d/expressions.rs` (accès `.champ`),
`src/sema/typecheck.rs` (`Expr::Field`, `Stmt::Assign`), `src/sema/unknown_field.rs`,
`src/sema/map_keys.rs`,
`src/lower/expr.d/lower.rs` et `src/lower/expr.d/helpers.rs`
(`is_map_target`, `lower_map_key`),
`src/lower/stmt.d/statements.d/assignments.rs`,
`src/lower/stmt.d/statements.d/map_loop_scan.rs`, `runtime/src/map_table.rs`,
`runtime/src/lib.rs` (`__map_get`, `__map_set`), `ci/perf/`, `docs/EBNF.md`,
`docs/builtins/Map.md`, `tools/`, `src/lsp/`.
