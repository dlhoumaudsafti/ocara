# `scoped`/`consumed` : deux points mineurs restants

Les 6 bugs de double-free/fuite/use-after-free d'origine, `Stmt::Result`, et la libération dans un bloc runtime (`main`/`init`/`exit`) sont tous corrigés — voir git log.

## Reste à faire : diagnostic mort `SemaError::OwnershipNotSupported`

Ce diagnostic n'est jamais émis en pratique (`scoped x:int`/`scoped f:SDL` sont acceptés sans avertissement, alors que le variant existe). À trancher : l'activer réellement (implique de valider l'impact sur les exemples existants qui pourraient s'appuyer sur ce silence) ou le retirer si le suivi de possession sur ces types n'a finalement pas de sens à interdire.

## Reste à faire (mineur, dépend d'un choix de design plus large) : `HTTPRequest`/`HTTPResponse` non typables `scoped`/`consumed`

Ces handles sont aujourd'hui de simples `int` (pas un vrai type nommé) — `scoped x:HTTPRequest` n'existe donc pas et ne peut pas exister dans le système de types actuel. Les rendre gérables par `scoped`/`consumed` demanderait d'abord d'en faire un vrai type, un changement d'API publique des builtins concernés à décider séparément.

## Fichiers clés

`src/sema/error.rs` (`OwnershipNotSupported`), `src/lower/stmt.d/ownership.rs`.
