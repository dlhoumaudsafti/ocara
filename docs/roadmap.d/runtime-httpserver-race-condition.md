# `HTTPServer` — data race documenté et non corrigé sur les closures capturées

## Constat

`runtime/src/httpserver.rs:35-37`, en commentaire d'en-tête, assumé noir sur blanc :

```
// Note sécurité concurrente :
//   Les captures partagées entre handlers (heap_promoted) ne sont pas
//   protégées par un mutex. Des accès concurrents constituent un data race.
```

`HTTPServer` tourne sur un pool de threads (`tiny_http`, N workers) — c'est un serveur réellement multi-thread, pas un mono-thread qui simule la concurrence. Une variable capturée par une closure `nameless` et partagée entre plusieurs handlers (cas d'usage courant : un compteur, un cache en mémoire, une connexion partagée) est donc exposée à un vrai data race dès que deux requêtes concurrentes touchent la même capture — pas un cas limite exotique, un scénario de base de tout serveur HTTP avec état partagé.

`HTTPServer` est un pilier de l'argument produit du README (« architecture web intégrée... serveur HTTP... ») et classé "Complet" dans l'analyse de la stdlib — mais cette classification ne tient pas compte de cette race, qui est un vrai défaut de fiabilité, pas une limitation de fonctionnalité.

## Ce qui est demandé

Deux niveaux de correction possibles, du moins au plus coûteux :
1. **Documenter clairement dans `docs/builtins/HTTPServer.md`** (actuellement silencieux sur ce point) que toute capture partagée entre handlers doit être protégée manuellement par `Mutex`/`m.withLock(...)` côté utilisateur — a minima, transformer un piège silencieux en piège documenté, avec un exemple correct dans la doc.
2. **Protéger nativement `heap_promoted`** par une synchronisation (mutex interne au runtime, ou primitive atomique selon le type de donnée) pour que le comportement par défaut soit sûr sans discipline particulière côté utilisateur Ocara — cohérent avec le fait qu'aucun mot-clé du langage n'indique aujourd'hui à l'utilisateur qu'une capture de closure passée à `HTTPServer::route`/`listen` doit être traitée différemment d'une capture dans un `Thread`.

Ajouter un test de régression qui exerce des requêtes concurrentes sur un état partagé (actuellement absent — `examples/advanced/httpserver`/`mini_project` n'exercent jamais la concurrence réelle malgré le pool multi-thread sous-jacent).

## Priorité / Complexité

**Priorité Haute** — race condition reproductible en usage normal (pas un cas limite) sur un builtin central de l'argument produit du langage. **Complexité : Structurel** pour l'option 2 (changement du runtime `HTTPServer`) — commencer par l'option 1 (documentation), rapide et sans risque, avant de décider si l'option 2 est nécessaire.

## Fichiers clés

`runtime/src/httpserver.rs`, `docs/builtins/HTTPServer.md`, `examples/advanced/httpserver/`, `examples/advanced/mini_project/`.
