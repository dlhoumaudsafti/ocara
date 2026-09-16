# `HTTPServer` — data race documenté et non corrigé sur les closures capturées

## Terminé — Option 1 (documentation + test) ET Option 2 (protection native) faites

Voir §"Ce qui a été fait" plus bas.

## Constat

`runtime/src/httpserver.rs:35-37`, en commentaire d'en-tête, assumé noir sur blanc :

```
// Note sécurité concurrente :
//   Les captures partagées entre handlers (heap_promoted) ne sont pas
//   protégées par un mutex. Des accès concurrents constituent un data race.
```

`HTTPServer` tourne sur un pool de threads (`tiny_http`, N workers) — c'est un serveur réellement multi-thread, pas un mono-thread qui simule la concurrence. Une variable capturée par une closure `nameless` et partagée entre plusieurs handlers (cas d'usage courant : un compteur, un cache en mémoire, une connexion partagée) est donc exposée à un vrai data race dès que deux requêtes concurrentes touchent la même capture — pas un cas limite exotique, un scénario de base de tout serveur HTTP avec état partagé.

`HTTPServer` est un pilier de l'argument produit du README (« architecture web intégrée... serveur HTTP... ») et classé "Complet" dans l'analyse de la stdlib — mais cette classification ne tient pas compte de cette race, qui est un vrai défaut de fiabilité, pas une limitation de fonctionnalité.

## Ce qui a été fait

### 1. ✅ Documentation (`docs/builtins/HTTPServer.md`)

Section « Notes de sécurité / concurrence » réécrite : explique pourquoi le parallélisme est réel et permanent (pas comme `Thread::spawn`, déclenché explicitement), donne un exemple ❌ (compteur non protégé, incrémentation perdue) puis un exemple ✅ (même compteur protégé par `ocara.Mutex`), et précise ce qui n'a PAS besoin de protection (tout ce qui est propre à une requête — chaque `req` a son propre `OcaraHttpContext`).

### 2. ✅ Exemple + test de régression (`examples/builtins/httpserver.oc`/`.sh`)

Route `/hits` ajoutée : compteur de visites partagé, protégé par un `Mutex` (`var`, pas `scoped` — capturé par le handler, doit survivre à tout le cycle de vie du serveur, même patron que `examples/builtins/mutex.oc`). `httpserver.sh` étendu : 30 requêtes lancées en VRAIE concurrence (`curl ... &` en parallèle, `wait` explicitement sur les PID des `curl` — pas un `wait` nu, qui attendrait aussi le serveur lui-même, jamais terminé) sur `/hits`, puis vérification que le compteur final vaut exactement `N + 1` (aucune incrémentation perdue). Vérifié directement : sous charge réelle (200 requêtes simultanées lors du test manuel avant de fixer la taille définitive à 30 pour la suite CI), le compteur protégé n'a jamais perdu une seule incrémentation. `make regression` : 637 PASS, 0 FAIL — `OK httpserver` inclus.

### ⚠️ Découverte en écrivant l'exemple — bug distinct, sans rapport avec la race condition elle-même

La première version de l'exemple utilisait `hitLock.withLock(nameless(): void { hitCount = hitCount + 1 })` **à l'intérieur** du handler `nameless(req:int): int {...}` — le patron recommandé par `docs/builtins/Mutex.md`. Ça ne fonctionnait pas silencieusement (`hitCount` restait à `0`) : bug de fermeture imbriquée sans rapport avec la concurrence, isolé et documenté séparément dans [langage-nested-closure-recapture](langage-nested-closure-recapture.md). **✅ Ce bug est maintenant corrigé** (voir cette fiche) — `examples/builtins/httpserver.oc` utilise à nouveau `withLock` imbriqué (le contournement `lock()`/`unlock()` manuels retiré), revérifié par le même test de charge concurrente.

## Option 2 — protection native : FAITE (sérialisation ciblée de l'invocation des handlers)

L'option 2 telle que formulée initialement (« protéger nativement `heap_promoted` par une synchronisation... pour que le comportement par défaut soit sûr ») s'est avérée, à l'examen, mal cadrée : `heap_promoted` n'est pas spécifique à `HTTPServer` — c'est le mécanisme général de capture de fermeture, partagé par `Thread::spawn` et tout usage de `nameless`. Un mutex global autour de chaque accès `heap_promoted` protégerait aussi bien `Thread` que `HTTPServer`, mais au prix d'un verrou sur CHAQUE lecture/écriture de CHAQUE variable capturée dans TOUT programme Ocara, y compris les closures jamais partagées entre threads (l'écrasante majorité des usages) — un coût de performance généralisé pour fermer un risque qui ne concerne que le sous-ensemble des captures réellement partagées. Rejetée.

Option retenue à la place, ciblée et sans toucher au mécanisme général de fermeture : sérialiser l'INVOCATION du handler dans `handle_request` (`runtime/src/httpserver.rs`) avec un mutex PAR SERVEUR (`handler_lock: Arc<Mutex<()>>`, créé une fois dans `HTTPServer_run`, cloné dans chaque thread worker). Nouvelle fonction `call_handler_locked` qui prend le verrou avant d'appeler le handler et le relâche (Drop du guard) juste après — appliquée aux TROIS sites d'appel de handler Ocara (route normale, handler d'erreur 404 après échec de fichier statique, handler d'erreur 404 sur méthode non-GET), pas seulement le premier : un handler d'erreur est un handler Ocara comme un autre, avec le même risque de capture partagée.

Élimine la race PAR CONSTRUCTION : deux handlers (route ou erreur) ne s'exécutent plus jamais en même temps sur un même serveur, quel que soit le nombre de `workers`. Coût assumé : perte du parallélisme réel sur la logique métier du handler (la lecture de la requête et l'envoi de la réponse, avant/après cet appel, restent parallèles). `HTTPServer` devient "sûr par défaut", alors que `Thread` reste "rapide par défaut, sûr sur demande (`Mutex`)" — incohérence de modèle assumée entre les deux primitives de concurrence du langage, documentée dans `docs/builtins/HTTPServer.md`.

**Analyse de risque `setjmp`/`longjmp` menée avant l'implémentation** (la même famille de risque que le ticket [exceptions-setjmp-longjmp-dette](exceptions-setjmp-longjmp-dette.md)) : un `raise` non rattrapé À L'INTÉRIEUR d'un handler pourrait en théorie sauter le `Drop` du `MutexGuard` (comme `longjmp` saute toute destruction normale) et laisser `handler_lock` verrouillé pour toujours. Vérifié que ce n'est PAS le cas : `TRY_STACK` est un thread-local (`runtime/src/lib.rs`), et un `raise` sans aucun `try` actif SUR CE THREAD (le thread worker qui exécute le handler) tombe dans la branche "aucun try actif" de `__ocara_fail`, qui termine tout le PROCESSUS (`std::process::exit(1)`) — le verrou n'a alors plus d'importance, le processus entier disparaît. Un `raise` RATTRAPÉ par un `try`/`on` interne au handler reste, lui, entièrement À L'INTÉRIEUR de l'appel à `f(...)` (le `longjmp` correspondant saute vers un point de la pile encore sous ce même appel) — `f(...)` retourne alors normalement, `_guard` est droppé normalement. Aucun scénario ne laisse `handler_lock` verrouillé indéfiniment.

Vérifié : `examples/builtins/httpserver.oc`/`.sh` mis à jour — la route `/hits` n'utilise plus de `Mutex` explicite (devenu redondant : la sérialisation native suffit désormais), toujours testée sous charge réelle (30 requêtes concurrentes, compteur final exact). `docs/builtins/HTTPServer.md`, section « Notes de sécurité / concurrence », réécrite : explique la sérialisation native, garde `Mutex` comme nécessaire uniquement pour l'état partagé avec du code hors invocation de handler (typiquement un `Thread` de fond), avec un nouvel exemple `HTTPServer` + `Thread` + `Mutex` illustrant ce cas restant. `make regression` : 659 PASS, 0 FAIL, `OK httpserver` inclus (build release avec `RUSTFLAGS="-D warnings"`, aucun warning).

## Priorité / Complexité

**Terminé.** Option 1 (documentation + test) et option 2 (sérialisation native) toutes deux faites — race condition éliminée par construction, plus seulement mitigée par la documentation. **Complexité réelle : Légère** — contrairement à l'estimation initiale ("Structurel"), une fois l'option correctement recadrée (mutex ciblé sur l'invocation du handler, pas sur `heap_promoted` en général), l'implémentation tient en un petit nombre de lignes localisées à `runtime/src/httpserver.rs`.

## Fichiers clés

`runtime/src/httpserver.rs` (`handler_lock`, `call_handler_locked`, fait), `docs/builtins/HTTPServer.md` (fait), `examples/builtins/httpserver.oc`/`.sh` (fait), `examples/advanced/httpserver/`, `examples/advanced/mini_project/`, [langage-nested-closure-recapture](langage-nested-closure-recapture.md) (bug connexe découvert et corrigé), [exceptions-setjmp-longjmp-dette](exceptions-setjmp-longjmp-dette.md) (même famille de risque `setjmp`/`longjmp`, analysée ci-dessus).
