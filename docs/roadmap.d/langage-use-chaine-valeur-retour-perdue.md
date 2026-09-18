# Appel chaîné cassé sur un récepteur qui n'est pas une variable nommée — `use Classe(...)`, ou `HTTPRequest::get(...)`

## Terminé — cause racine trouvée et corrigée

Voir §"Ce qui a été fait" plus bas.

## Constat (avant correctif)

Repéré en écrivant `examples/tests/52_class_resource_property_destructorTest.oc` (chantier du destructeur de champ ressource, voir [langage-destructeur-champ-ressource](langage-destructeur-champ-ressource.md)) — **sans aucun rapport avec ce chantier-là**, reproduit avec une classe qui ne contient même pas de ressource :

```ocara
import ocara.IO

class Bar {
    init() {
    }
    public method getIt(): int {
        return 777
    }
}

function main(): int {
    var r1:int = use Bar().getIt()
    IO::writeln(`r1 = ${r1}`)   // affichait 0, pas 777
    return 0
}
```

Second symptôme, plus grave, dans `examples/advanced/tauri_httpserver/main.oc` : `use Thread().run(nameless(): void { ... })` chaîné ne lançait JAMAIS le thread — sa closure ne s'exécutait jamais, sans la moindre erreur de compilation.

Les deux symptômes partageaient la même signature : `use Classe(args).méthode()` **chaîné en une seule expression, sans jamais lier l'instance à une variable**. Séparer en deux étapes (`var t:Classe = use Classe(); t.méthode()`) fonctionnait toujours correctement — c'était d'ailleurs le seul contournement disponible avant ce correctif.

## Cause racine

`use Classe(...)` se parse en un simple `Expr::New` (`src/parsing/parser.d/expressions.rs`, `TokenKind::Use`) — un `use Classe(...).méthode()` chaîné est donc un `Expr::Call` dont le récepteur (`object`) est **directement** un `Expr::New`, jamais un `Expr::Ident`.

Or **neuf points de résolution de classe du récepteur**, répartis dans quatre fichiers du lowering, ne géraient que `Expr::Ident` (variable nommée) / `Expr::SelfExpr` / `Expr::ParentExpr` / `Expr::Field` (accès chaîné) — jamais `Expr::New` :

- `src/lower/expr.d/lower.rs` : résolution de classe pour un appel de méthode (`Expr::Call` → `Expr::Field`) et pour un accès de champ (`Expr::Field` seul).
- `src/lower/expr.d/typeinfer.rs` : mêmes deux résolutions, côté inférence du type IR de retour (utilisée pour le boxing).
- `src/lower/expr.d/helpers.rs` : `resolve_chained_field_class` (accès chaîné à 2+ niveaux, ex. `use Foo().inner.method()`), et `is_map_target`/`elem_type_after_index` (indexation chaînée sur un champ `array`/`map`).
- `src/lower/stmt.d/statements.d/assignments.rs` : écriture d'un champ (`use Foo().champ = ...`) et `++`/`--` sur un champ.
- `src/lower/builder.d/message_gen.rs` : détection d'un appel à un générateur (`emit`) en récepteur chaîné.

Pour un appel de méthode (le cas de `use Bar().getIt()`/`use Thread().run(...)`), l'absence du cas `Expr::New` faisait tomber la résolution dans le filet de sécurité générique : *"peut-être une string"* (`expr_ir_type(object) == IrType::Ptr` → devine `"String"`, puisque `Expr::New` est TOUJOURS `Ptr` au niveau IR — voir sa doc dans `typeinfer.rs`). `func_mangled` valait alors `"String_getIt"`/`"String_run"` — un symbole qui n'existe jamais. Le codegen (`src/codegen/emit.d/instructions.d/calls.rs`, `emit_calls`) **ignore silencieusement un appel vers une fonction inconnue** au lieu d'échouer (commentaire d'origine : *"Si la fonction n'est pas connue, on ignore (runtime résolution)"*, pensé pour un dispatch dynamique légitime ailleurs) — d'où le symptôme : pour un retour scalaire, la destination SSA jamais définie valait `0` ; pour `Thread::run` (`void`), l'appel entier disparaissait silencieusement, donc le thread n'était jamais lancé.

Un cas (`raise use FileNotFound(...)`, `src/lower/stmt.d/statements.d/exceptions.rs:26`) traitait DÉJÀ `Expr::New` correctement — la preuve que le bon pattern existait déjà dans la base de code, juste pas appliqué partout où il fallait.

### Troisième symptôme, cause voisine mais distincte : `HTTPRequest::get(...).ok()` chaîné

Repéré juste après avoir corrigé les neuf sites `Expr::New` ci-dessus, en revérifiant `examples/advanced/tauri_httpserver/main.oc` — signalé indépendamment par l'utilisateur au même moment :

```ocara
if HTTPRequest::get("http://localhost:8080/health").ok() {   // toujours faux, même si le serveur répond 200
```
```ocara
consumed resp:HTTPResponse = HTTPRequest::get("http://localhost:8080/health")
if resp.ok() { ... }   // correct
```

Cause DIFFÉRENTE de celle des neuf sites `Expr::New` : `Expr::StaticCall` (l'AST de `Classe::méthode(...)`) est un nœud **complet en lui-même** — ses arguments sont intégrés directement au nœud (`Expr::StaticCall { class, method, args, .. }`), IL N'EST JAMAIS enveloppé dans un `Expr::Call` séparé (contrairement à ce qu'on pourrait supposer par analogie avec un appel de méthode d'instance — confirmé en lisant `parse_postfix`/`parse_primary`, `src/parsing/parser.d/expressions.rs`). Donc pour `HTTPRequest::get(url).ok()`, le récepteur (`object`) de `.ok()` est **directement** `Expr::StaticCall`, jamais `Expr::Call{callee: Expr::StaticCall}`.

Erreur de correctif initiale, corrigée dans la foulée : le premier essai ajoutait le cas `Expr::StaticCall` **à l'intérieur** de la branche `Expr::Call { callee: inner_callee, .. }` existante (en supposant, à tort, la même structure imbriquée que `Expr::New`) — ce code n'était jamais atteint, puisque `object` ne matchait jamais `Expr::Call` pour ce cas. Repositionné comme branche directe et indépendante du `match object.as_ref()`, aux deux mêmes fichiers (`lower.rs`, `typeinfer.rs`), restreint aux raccourcis `HTTPRequest::get/post/put/delete/patch` (qui retournent tous `HTTPResponse`).

## Ce qui a été fait

- Ajout de `Expr::New { class, .. } => Some(class.clone())` aux neuf points de résolution listés ci-dessus (avant tout fallback générique, pour ne jamais tomber dans une devinette).
- Ajout de `Expr::StaticCall { class, method, .. } if class == "HTTPRequest" && méthode ∈ {get, post, put, delete, patch} => Some("HTTPResponse")` comme branche directe du même `match` de résolution du récepteur, dans `lower.rs` et `typeinfer.rs`.

### Vérifications

- 3 nouveaux tests Rust unitaires : `resolve_chained_field_class_recognizes_new_expr_as_base` (`src/lower/expr.d/tests.rs`).
- Nouvel exemple de régression `examples/tests/53_use_chained_methodTest.oc` (5 assertions) : retour scalaire (`int`/`string`), constructeur avec/sans argument, lecture d'un champ chaîné, et surtout un **vrai thread OS** lancé via `use Thread().run(...)` chaîné, vérifié avec un `Mutex` partagé (même patron que `examples/builtins/httpserver.oc`) — confirme que le thread s'exécute réellement, pas seulement que la compilation réussit.
- Nouvel exemple `examples/tests/54_httprequest_chained_methodTest.oc` (2 assertions) : `HTTPRequest::get(...).ok()`/`.status()` chaînés contre un vrai `HTTPServer` local, vérifie `.ok() == true` ET `.status() == 200` (pas seulement un contournement partiel du symptôme).
- `cargo test -p ocara` : 87 passed. `make regression` (cache vidé) : 684 PASS, 0 FAIL, 0 ERREUR. `make build` : 0 warning.
- `examples/advanced/tauri_httpserver/main.oc` : le contournement (variable nommée + `.detach()` pour `Thread`, `consumed resp:HTTPResponse` pour la boucle d'attente) retiré, code chaîné d'origine restauré, compile et **fonctionne réellement** (`Serveur prêt.` s'affiche, health-check réussi dès la première tentative).

## Fichiers clés

`src/lower/expr.d/lower.rs`, `src/lower/expr.d/typeinfer.rs`, `src/lower/expr.d/helpers.rs`, `src/lower/stmt.d/statements.d/assignments.rs`, `src/lower/builder.d/message_gen.rs` (les neuf sites `Expr::New`, plus les deux sites `Expr::StaticCall` dans `lower.rs`/`typeinfer.rs`), `src/parsing/parser.d/expressions.rs` (`parse_postfix`/`parse_primary`, pour comprendre la forme AST exacte d'un `Expr::StaticCall` chaîné), `src/codegen/emit.d/instructions.d/calls.rs` (`emit_calls`, où l'appel vers un symbole inconnu était silencieusement ignoré — comportement inchangé, la vraie cause était en amont), `src/lower/expr.d/tests.rs`, `examples/tests/53_use_chained_methodTest.oc`, `examples/tests/54_httprequest_chained_methodTest.oc`, `examples/advanced/tauri_httpserver/main.oc`.
