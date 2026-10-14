# Transactions SQLite / MySQL limitées à une seule instruction — virement non atomique

Statut : **corrigé** le 2026-10-13 (constaté le même jour par l'audit de la roadmap).

## Constat

`prepare`/`bind`/`commit`/`rollback` sont présentés comme des transactions
(« réussir ou échouer en bloc », `docs/builtins/SQLite.md` ~l.119-132,
`docs/builtins/MySQL.md`), mais chaque `commit()` ouvre et ferme son propre
`BEGIN … COMMIT`. L'exemple « virement » de la documentation n'est pas
atomique.

Reproduction (SQLite) : crédit de 100 sur le compte 2 validé, puis le débit du
compte 1 échoue (contrainte `CHECK`), `rollback()` appelé :

```
id=1 balance=500
id=2 balance=100      // 100 créés à partir de rien
```

MySQL : même conception d'après le code (non exécuté, pas de serveur).

## Correction attendue

Une vraie transaction englobant plusieurs instructions : `begin()` explicite
(ou `commit` qui regroupe toutes les instructions préparées depuis le dernier
`commit`/`rollback`), `rollback()` qui annule l'ensemble. Aligner la
documentation et l'exemple.

## Validation

Tests runtime (`runtime/src/tests/sqlite.rs`) et `examples/tests/` sur un
virement dont la seconde instruction échoue (aucune ligne modifiée) ; même
test MySQL marqué `#[ignore]` sans serveur.

## Priorité / Complexité

**Haute** (intégrité des données, la documentation promet l'inverse) —
**Structurel**.

## Correctif

Choix retenu : **`begin()` explicite**. `db.begin()` ouvre une transaction : toutes les instructions suivantes (`execute`, `query`, `queryOne`, requête préparée) en font partie jusqu'au `commit()` (validation de l'ensemble, requête préparée éventuelle comprise) ou au `rollback()` (annulation de l'ensemble). Sans `begin()`, `commit()` garde son comportement d'origine. SQLite : état `explicit_tx` (`runtime/src/sqlite.rs`) ; MySQL : la transaction vit sur une connexion épinglée, utilisée par toutes les requêtes tant qu'elle est ouverte (`acquire_conn`, `runtime/src/mysql.rs`). Vérifié : le virement documenté est désormais atomique (échec → solde inchangé). Tests : `sqlite_begin_*` (`runtime/src/tests/sqlite.rs`) ; MySQL non exécuté (pas de serveur de test).
