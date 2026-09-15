# `scoped`/`consumed` : un point mineur restant

Les 6 bugs de double-free/fuite/use-after-free d'origine, `Stmt::Result`, la libération dans un bloc runtime (`main`/`init`/`exit`), et le diagnostic mort `SemaError::OwnershipNotSupported` (retiré : jamais construit nulle part, et le code documentait déjà explicitement que `scoped`/`consumed` sur un type non pris en charge doit se comporter comme `var`, sans erreur — l'activer aurait cassé cet usage établi) sont tous traités — voir git log.

## Reste à faire (mineur, dépend d'un choix de design plus large) : `HTTPRequest`/`HTTPResponse` non typables `scoped`/`consumed`

Ces handles sont aujourd'hui de simples `int` (pas un vrai type nommé) — `scoped x:HTTPRequest` n'existe donc pas et ne peut pas exister dans le système de types actuel. Les rendre gérables par `scoped`/`consumed` demanderait d'abord d'en faire un vrai type, un changement d'API publique des builtins concernés à décider séparément.

## Fichiers clés

`src/builtins/httpserver.rs` (ou équivalent), système de types (`src/parsing/ast.d/types.rs`).
