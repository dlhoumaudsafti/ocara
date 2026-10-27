# Clé littérale sur une grande map : hachage à chaque accès

Statut : **à étudier** — ouverte le 2026-10-27, suite de l'étude
d'accélération menée avec la notation pointée (`truc.A` ≡ `truc["A"]`,
`docs/EBNF.md` §7.4).

## Déjà fait (2026-10-27)

Petites maps (jusqu'à `SMALL` = 8 paires, sans index —
`runtime/src/map_table.rs`) :

- `__map_get` cherche d'abord la clé **par pointeur** parmi les clés rangées
  (`MapTable::stored_index`) : un littéral interné (`m["A"]`, `m.A`) rangé
  depuis le même littéral est trouvé sans lire aucun texte ;
- `find_small` (écriture, `has`…) fait la même passe avant la comparaison
  de texte ;
- `MapTable::insert` reçoit la chaîne clé empruntée (`shared`) et ne la
  retient qu'en créant une paire : plus de retenue suivie d'un relâchement
  à chaque `m["k"] = v` sur une clé existante.

Mesure `mapconst` (8 clés, 3 000 000 tours, 4 lectures et 2 écritures par
tour) : voir les chiffres consignés dans le message de commit et
`make perf` ; lectures seules 0,15 s → 0,10 s (PHP 0,18 s).

## Reste

Au-delà de 8 paires, la table a un index : une clé littérale est hachée à
chaque accès (`hash_text`, graine propre à chaque map), puis sondée. Pistes :

- **cache par site d'appel** : rang de la paire trouvée au dernier accès,
  vérifié par pointeur avant tout hachage (comme `__map_value_at` pour
  `for k has v in m`) — demande une case mutable par site, émise par le
  codegen ;
- **hachage précalculé** d'une clé littérale : impossible tel quel (graine
  par map) ; à évaluer avec une graine par processus réservée aux clés
  littérales, sans affaiblir la résistance aux clés hostiles venues du
  réseau.

Mesure à ajouter avant tout changement : variante de `mapconst` sur une map
de 64 clés.

## Complexité

Structurel (case par site émise par le codegen) — Légère pour la mesure.
