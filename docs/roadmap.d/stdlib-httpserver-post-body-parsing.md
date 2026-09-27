# Pas de parseur `application/x-www-form-urlencoded` pour le corps d'une requête POST

Vérifié :
- **Fondu dans le même chantier que** [stdlib-httpserver-request-object](stdlib-httpserver-request-object.md) (désormais clos), comme anticipé par ce ticket lui-même ("voir aussi... si traités dans la même passe, puisque les trois touchent `configs/routes/*.oc`/les controllers des mêmes exemples") — voir ce ticket pour le détail complet (tests, fichiers, corpus migré) ; ce fichier documente uniquement la partie spécifique à ce ticket.
- `parse_query`/`url_decode` (`runtime/src/httpserver.rs`) — la logique déjà éprouvée pour la query string — sont réutilisées TELLES QUELLES contre le corps de la requête (pas dupliquées) dès que `Content-Type: application/x-www-form-urlencoded` est détecté, exactement comme proposé par ce ticket.
- Exposé via `req.param(key)`/`req.params()` (voir `HTTPServerRequest`, [stdlib-httpserver-request-object](stdlib-httpserver-request-object.md)) plutôt que l'accessor dédié `bodyParam(req, key)` initialement envisagé ici — décision prise dans le chantier plus large : un accesseur UNIFIÉ (query string + corps, quel que soit son format) est plus utile qu'un accesseur séparé par source, et couvre aussi bien `urlencoded` que le nouveau `multipart/form-data` (hors périmètre de ce ticket-ci, ajouté séparément) sans multiplier les points d'entrée.
- Vérifié bout-en-bout (`examples/tests/59_httpserver_requestTest.oc`, méthode `postUrlencodedParamOverridePrecedenceTest`) : un vrai `POST` avec corps `application/x-www-form-urlencoded` contre un vrai serveur, lu via `param()`/`params()` — y compris le cas de collision de clé entre la query string de l'URL et le corps (le corps l'emporte, règle de précédence documentée dans le ticket parent).
- Testé unitairement côté Rust (`runtime/src/tests/httpserver.rs::build_params_buckets_urlencoded_body_in_actual_method_bucket` et les tests de fusion GET+corps) — le corps urlencoded parse correctement, séparément de la query string, avec la bonne règle de précédence.
- Migration GET→POST des formulaires de `mini_project`/`mini_project_hexa` mentionnée dans la "Proposition" originale de ce ticket : **non traitée** dans ce chantier — la migration effectuée (voir le ticket parent) est purement l'API `req:int`→`HTTPServerRequest`, aucune route existante n'a changé de méthode HTTP. Cette migration GET→POST reste possible dès maintenant (le parseur urlencoded existe) mais n'a pas été demandée explicitement dans le périmètre de ce chantier — à traiter séparément si souhaité.

## Constat (ticket original)

`HTTPServer::query(req, key)` ne décodait que la query string de l'URL — le corps de la requête était lu tel quel, jamais parsé. Conséquence concrète : dans `examples/advanced/mini_project` et `mini_project_hexa`, tous les formulaires de soumission utilisaient `GET` au lieu de `POST`, signalé explicitement en commentaire dans le code, précisément à cause de cette absence de parseur.

Le corps JSON était déjà exploitable sans changement runtime (`JSON::decode(req.body())`) — resté vrai, explicitement hors périmètre de `param`/`params` (voir le ticket parent).

## Priorité / Complexité

**Terminé.** Était Simple à Légère ("le plus simple des trois tickets de cette série") — confirmé : `parse_query`/`url_decode` réutilisées sans aucune modification, tout le travail neuf de ce ticket s'est limité à les appliquer au corps plutôt qu'à la query string, dans `build_params_buckets` (voir le ticket parent pour le détail de cette fonction, partagée avec le support multipart ajouté dans le même chantier).

## Fichiers clés

`runtime/src/httpserver.rs` (`build_params_buckets`, réutilise `parse_query`/`url_decode` sans les modifier), `examples/tests/59_httpserver_requestTest.oc`. Voir [stdlib-httpserver-request-object](stdlib-httpserver-request-object.md) pour la liste complète des fichiers touchés par le chantier global.
