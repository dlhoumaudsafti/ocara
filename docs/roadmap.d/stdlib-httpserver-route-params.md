# Paramètres de route dynamiques (`/voitures/{id}` ou `/voitures/<id:int>`)

## Constat

`server.route(path, method, handler)` ne fait aujourd'hui qu'une comparaison de chaîne stricte (`runtime/src/httpserver.rs:357-359` : `r.path == path || r.path == "*"`) — `path`/`method` sont de simples `String`, jamais découpées en segments à l'enregistrement (`HTTPServer_route`, `:519-536`). Le seul cas non littéral est `"*"`, un joker total par méthode (aucune extraction possible), non documenté et jamais utilisé dans le corpus.

Conséquence dans tout le corpus existant : un identifiant de ressource passe par la query string plutôt que par le chemin (`/voitures/detail?id=1` plutôt que `/voitures/1` ou `/voitures/detail/1`), voir `CarController::show` (`HTTPServer::query(req, "id")`).

## Proposition

Ajouter la capture de segments dynamiques dans le chemin de route, avec extraction automatique et typée. Deux syntaxes possibles :

- `/voitures/{id:int}` — accolades, courant dans beaucoup de frameworks web (Flask, Express-like).
- `/voitures/<id:int>` — chevrons, plus proche de la syntaxe déjà utilisée ailleurs dans Ocara pour les génériques (`array<T>`, `map<K,V>`) et les unions.

**Avis exprimé lors de la discussion initiale : `<id:int>` est plus proche de l'esprit syntaxique d'Ocara** — mais c'est une décision à trancher explicitement avant d'implémenter, pas encore actée.

## Ce qu'il faut trancher

- Syntaxe exacte (`{}` vs `<>`) et son support pour d'autres types que `int` (`string` au minimum).
- Comportement si un paramètre déclaré n'est pas convertible (ex. `/voitures/{id:int}` appelé avec `/voitures/abc`) — 404 ? 400 ? erreur non gérée qui remonte au handler ?
- Priorité entre route statique et route dynamique quand les deux pourraient matcher le même chemin.
- Accès à la valeur côté Ocara : nouvelle méthode (ex. `HTTPServer::param(req, "id")`, ou intégré à l'objet requête si [stdlib-httpserver-request-object](stdlib-httpserver-request-object.md) est traité en même temps).

## Complexité

**Légère** pour une capture simple à un seul type par segment (pas de regex, pas de règles de priorité route statique/dynamique) — confiné à `runtime/src/httpserver.rs` (compilation du chemin en segments, nouveau champ `params` sur `OcaraHttpContext`) plus l'enregistrement builtin habituel (`src/builtins/httpserver.rs`, `src/codegen/desc.d/httpserver.rs`). Devient Structurel si la syntaxe retenue doit gérer plusieurs segments dynamiques, des types autres qu'int/string, ou des règles de priorité explicites entre routes statiques et dynamiques.

## Fichiers clés

`runtime/src/httpserver.rs` (matching des routes), `src/builtins/httpserver.rs`, `src/codegen/desc.d/httpserver.rs`, `docs/builtins/HTTPServer.md`, `docs/EBNF.md`.
