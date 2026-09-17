# `SQLite`/`MySQL` n'exposent aucune requête paramétrée — l'échappement anti-injection retombe sur le code utilisateur

## Constat

`db.execute(query: string)`, `db.query(query: string)` et `db.queryOne(query: string)` (`docs/builtins/SQLite.md:40,53,69` ; mêmes signatures côté `docs/builtins/MySQL.md`) n'acceptent qu'une chaîne SQL déjà entièrement construite — aucune variante `execute(query, params)`/bind par placeholder n'existe dans la stdlib. Toute valeur variable (saisie utilisateur, paramètre de route HTTP...) doit donc être concaténée à la main dans la requête.

La preuve la plus directe que ce n'est pas un détail théorique : `examples/advanced/mini_project/helpers/SqlSafe.oc` est une classe entière du projet d'exemple "avancé" qui n'existe que pour ça — son propre commentaire le dit explicitement : *« `ocara.SQLite` n'expose pas de requêtes préparées/paramétrées [...] donc toute valeur variable concaténée dans une requête DOIT passer par `SqlSafe::escape()` pour éviter l'injection SQL »*. `models/Car.oc` du même exemple (`examples/advanced/mini_project/models/Car.oc:28-43`) construit ses `INSERT`/`UPDATE` par concaténation de chaînes échappées à la main, y compris pour un champ `float` (`Convert::floatToStr`) et un timestamp — un seul appel `SqlSafe::escape()` oublié sur un futur champ texte suffit à réintroduire une injection SQL, et rien dans le compilateur ni le runtime ne peut le détecter.

Pour un langage dont l'argument de vente est une "architecture web intégrée" (serveur HTTP natif, ORM absent mais accès SQL direct encouragé), demander à chaque utilisateur de réimplémenter son propre échappement est un vrai manque de maturité de la stdlib, pas juste un détail d'ergonomie.

## Ce qui est demandé

Étudier l'ajout d'une forme paramétrée aux méthodes `execute`/`query`/`queryOne` de `SQLite` et `MySQL`/`MariaDB` — par exemple `db.execute(query: string, params: array<mixed>)` avec placeholders `?` (convention SQLite/MySQL native, déjà supportée par les bibliothèques C sous-jacentes), qui échapperait/bindrait chaque valeur correctement selon son type au lieu de faire de l'interpolation de chaîne. Objectif : que le chemin "sûr par défaut" soit aussi le chemin le plus court, pas un helper à écrire soi-même à chaque projet.

Périmètre à clarifier avant implémentation : binding par position uniquement (plus simple, suffisant pour la majorité des cas) vs binding nommé ; impact sur `docs/builtins/SQLite.md`/`MySQL.md` (nouvelle section + mise à jour des exemples) ; devenir de l'exemple `SqlSafe.oc` une fois la vraie solution disponible (à retirer/remplacer, pas à laisser comme deux façons concurrentes de faire la même chose).

## Priorité / Complexité

**Priorité Moyenne** — n'affecte pas la stabilité/correction du compilateur (donc ne bloque pas la définition "le langage est stable"), mais c'est un vrai manque de sécurité par défaut dans une brique standard mise en avant par le projet. **Complexité Structurel** — nouvelle surface d'API sur deux builtins (`SQLite`, `MySQL`/`MariaDB`), touche le binding C sous-jacent (`runtime/`) côté préparation/binding de requête, pas seulement la couche Ocara.

## Fichiers clés

`docs/builtins/SQLite.md`, `docs/builtins/MySQL.md`, `runtime/` (bindings SQLite/MySQL C), `src/builtins/` (déclarations des méthodes), `examples/advanced/mini_project/helpers/SqlSafe.oc` (à faire évoluer une fois la vraie solution en place), `examples/builtins/sqlite.oc`, `examples/builtins/mysql.oc`.
