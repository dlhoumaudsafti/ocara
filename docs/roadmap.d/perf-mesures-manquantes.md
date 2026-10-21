# Performances : mesures manquantes dans `make perf`

Relevé le 2026-10-21. `make perf` (`ci/perf_python.sh`, programmes
`ci/perf/<nom>.oc` + `<nom>.py`) compare 17 mesures à Python et échoue si
Ocara est plus lent — règle : **aucune mesure plus lente que Python**
(`docs/benchmarking.md`). Les mesures actuelles couvrent les chaînes, les
tableaux, les maps à clés `string`, les objets, les méthodes, les gabarits,
les exceptions, `async` et JSON. Les points ci-dessous n'y figurent pas.

Pour chacun : ajouter la paire `ci/perf/<nom>.oc` / `<nom>.py` et la taille
dans `BENCHES` (`ci/perf_python.sh`), mesurer, puis corriger le runtime ou le
codegen si Ocara est derrière.

## 1. Points faibles probables (à ajouter en premier)

Classés par risque d'être plus lent que Python, d'après le fonctionnement
connu du runtime — **aucun n'a encore été mesuré**.

| # | Mesure | Pourquoi c'est risqué | Référence Python |
|---|---|---|---|
| 1 | **Appel via une interface** (10⁷ appels d'une méthode d'interface) | ≈ 50 ns par appel (dispatcher + retenue de `self`), seul surcoût connu au-dessus de Python (méthode Python ≈ 37 ns) | appel de méthode sur une classe de base |
| 2 | **Closures et valeurs `Function`** (nameless qui modifie un compteur capturé, appelée 10⁷ fois) | toute variable capturée vit dans une cellule verrouillée (`__locked_cell_get`/`__locked_cell_set`) | fonction imbriquée avec `nonlocal` |
| 3 | **`map<int, V>`** (10⁶ insertions + lectures) | clés stockées en texte : chaque accès par entier formate une chaîne (`key_to_string`) | `dict` à clés `int` |
| 4 | **Générateurs** (`emit` + `for`, 10⁷ valeurs) | un appel de reprise (`<nom>__resume`) et des allocations par valeur produite | générateur `yield` |
| 5 | **Arithmétique `mixed` / unions `int|float`** (10⁷ opérations) | chaque opération passe par `__dyn_add`/`__dyn_sub`… et peut créer une valeur boxée | arithmétique Python ordinaire |
| 6 | **Mode multi-thread** (travail de `objs`/`arr` après le lancement d'un `Thread` vide) | dès qu'un thread Ocara a existé, comptes de références atomiques et verrou de chaque conteneur reviennent, définitivement | même boucle après `threading.Thread(...).start()` |
| 7 | **Exceptions de classe** (`raise use MonErreur(...)` rattrapée par `on e is MonErreur`, 10⁶ fois) | allocation d'un objet par exception (la mesure `exc` ne lève qu'une chaîne) | `raise MonErreur(...)` / `except MonErreur` |

## 2. Bibliothèque standard non couverte

| # | Mesure | Référence Python | Remarque |
|---|---|---|---|
| 8 | `Regex` : correspondance et remplacement en boucle | module `re` | |
| 9 | Fichiers : écrire puis relire 10⁶ lignes (`File`) | `open()` / itération | fichier temporaire, supprimé après |
| 10 | `Convert::strToInt` / `strToFloat` sur 10⁶ chaînes | `int()` / `float()` | |
| 11 | `Array::contains`, `indexOf`, `slice`, `reverse`, `pop` | méthodes de `list` | |
| 12 | Tri de chaînes et tri d'objets | `sorted()` | `sort` ne mesure que des entiers |
| 13 | `SQLite` : insertions + `query` en boucle | `sqlite3` | base en mémoire |
| 14 | `YAML::decode` | PyYAML | sauter la mesure si PyYAML n'est pas installé |
| 15 | `Date` / `Time` : formatage en boucle | `datetime` | |
| 16 | `Mutex` : `lock`/`unlock` sans contention | `threading.Lock` | |

## 3. Hors calcul (critères séparés)

Ne pas les mélanger aux mesures de temps de `make perf` — prévoir une
section ou une cible à part :

- **Temps de démarrage** : programme vide contre `python3 -c pass` (Ocara
  gagne largement — utile comme garde-fou).
- **Mémoire résidente maximale** à travail égal (`/usr/bin/time -f %M`) :
  second critère, distinct du temps.
- **Serveur HTTP** (requêtes par seconde) contre `http.server` : nécessite
  un client de charge, plus fragile pour une passe automatique.

## Priorité / Complexité

**Moyenne** (aucun échec connu de `make perf`, mais des cas probablement
plus lents que Python restent non mesurés) — **Légère** (ajout des paires
de mesure) ; la correction d'un point qui échoue peut être **Structurelle**
(cellules de capture sans verrou, clés `int` natives dans les maps,
dispatch d'interface).

## Fichiers clés

`ci/perf_python.sh`, `ci/perf/`, `docs/benchmarking.md`,
`runtime/src/lib.rs` (cellules, `key_to_string`, `__dyn_*`),
`runtime/src/containers.rs`, `runtime/src/rc.rs`,
`src/lower/builder.d/interfaces.rs`, `src/lower/builder.d/message_gen.rs`.
