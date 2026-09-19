# Un appel de méthode chaîné sur le résultat `void` d'un appel précédent compile sans erreur, et le reste de la chaîne est silencieusement ignoré

## Terminé — rejet à la compilation (E36), jamais de retour `self` implicite

Voir §"Ce qui a été fait" plus bas.

## Constat (avant correctif)

Découvert dans `examples/advanced/tauri_httpserver/configs/Server.oc`, en corrigeant les bugs de [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md) :

```ocara
self.port(8080)
    .workers(4)
    .rootPath("./public/")
```

`HTTPServer::port`/`workers`/`rootPath` retournent tous `void` (`docs/builtins/HTTPServer.md`) — ce ne sont PAS des méthodes fluides (aucune ne retourne `self`). Ce code **compilait sans la moindre erreur ni avertissement**, mais `workers(4)` et `rootPath(...)` n'étaient JAMAIS appelés — confirmé en conditions réelles : le serveur démarrait avec 4 workers par défaut (pas 4 volontaires, coïncidence de valeur par défaut) et **aucun `root_path` configuré** (`"[STATIC] No root_path configured"` dans les logs runtime), donc plus aucun fichier statique n'était jamais servi.

## Décision de design : pas de retour `self` implicite pour les méthodes `void`

Avant d'implémenter un correctif, une question de design a été posée : plutôt que de rejeter la chaîne, ne vaudrait-il pas mieux faire des méthodes comme `port`/`workers`/`rootPath` des méthodes fluides (`self` retourné implicitement quand le type de retour déclaré est `void`), pour que le style d'écriture d'origine fonctionne tel quel ?

**Rejeté.** Deux raisons données par l'utilisateur, retenues :
1. **Non-sens par rapport à l'objectif de typage strict du langage** — faire qu'un `void` déclaré renvoie silencieusement autre chose (`self`) à l'exécution contredit la promesse même du type `void` (« cette méthode ne produit aucune valeur ») ; ce n'est pas un raffinement du typage, c'est une exception cachée à la règle.
2. **Changement d'API systémique, pas un correctif ciblé** — ça obligerait à trancher, pour CHAQUE méthode `void` existante et future (tous les builtins compris), si elle est « fluide » ou « void strict », un choix qui n'a rien à voir avec le bug d'origine et qui casserait la convention actuelle (un appel par ligne, partout dans la doc et les exemples).
3. Argument technique supplémentaire : cette session a déjà trouvé et corrigé DEUX autres bugs distincts, tous liés au chaînage d'appels sur un récepteur qui n'est pas une variable nommée (voir [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md)) — encourager davantage de chaînage maintenant aurait exposé à plus de cette même famille de bugs plutôt que de la refermer.

## Ce qui a été fait

Nouveau diagnostic **E36** (`SemaError::MethodCallOnVoid`, `docs/diagnostics.md` §E36) : dans `src/sema/typecheck.rs`, au point exact où le récepteur d'un appel de méthode (`Expr::Call { callee: Expr::Field { object, field } }`) est typé, un `obj_ty == Type::Void` est maintenant explicitement rejeté — au lieu de retomber, comme n'importe quel autre type sans classe associée, dans le filet de sécurité permissif (`type_class_name` renvoie `None` → `Type::Mixed` silencieux).

Portée volontairement restreinte à ce cas précis (appel de méthode) — un accès de CHAMP simple sur un récepteur `void` (`expr.champ`, sans appel, cas très marginal en pratique) et l'indexation (`expr[i]`) n'ont pas été touchés, pour rester un correctif contenu et à faible risque plutôt que généraliser à toute utilisation d'un `void` comme valeur.

### Vérifications

- 3 nouveaux tests Rust unitaires (`src/sema/tests/method_call_on_void.rs`) : le cas exact du bug rejeté, les mêmes appels séparés toujours acceptés (le contournement), et un chaînage sur un retour NON-`void` (méthode qui retourne réellement `self`) toujours accepté — le rejet est spécifique à `void`, pas à tout chaînage.
- `make tests` : 101 passed. `make regression` (cache vidé) : 684 + 50 PASS, 0 FAIL, 0 ERREUR — confirme qu'aucun exemple existant du corpus ne reposait, même involontairement, sur ce chaînage désormais rejeté. `make build` : 0 warning.
- `examples/advanced/tauri_httpserver/configs/Server.oc` : le contournement (trois appels séparés) reste tel quel — c'est désormais la seule forme valide, plus seulement une préférence de style.

## Fichiers clés

`src/sema/error.rs` (`SemaError::MethodCallOnVoid`), `src/sema/typecheck.rs` (le rejet, dans la branche `Expr::Call`/`Expr::Field`), `src/sema/tests/method_call_on_void.rs` (nouveau), `docs/diagnostics.md` (§E36, nouveau), `examples/advanced/tauri_httpserver/configs/Server.oc`, [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md) (bugs voisins, même symptôme de fond : symbole mangled inexistant ignoré silencieusement par le codegen).
