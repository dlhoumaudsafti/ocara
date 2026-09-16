# `HTTPServer` — data race documenté et non corrigé sur les closures capturées

## Option 1 faite (documentation + test) — option 2 (protection native) en suspens, décision demandée

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

## Option 2 — protection native de `heap_promoted` : PAS entreprise, décision à prendre

L'option 2 telle que formulée initialement (« protéger nativement `heap_promoted` par une synchronisation... pour que le comportement par défaut soit sûr ») s'avère, à l'examen, mal cadrée : `heap_promoted` n'est pas spécifique à `HTTPServer` — c'est le mécanisme général de capture de fermeture, partagé par `Thread::spawn` et tout usage de `nameless`. Un mutex global autour de chaque accès `heap_promoted` protégerait aussi bien `Thread` que `HTTPServer`, mais au prix d'un verrou sur CHAQUE lecture/écriture de CHAQUE variable capturée dans TOUT programme Ocara, y compris les closures jamais partagées entre threads (l'écrasante majorité des usages) — un coût de performance généralisé pour fermer un risque qui ne concerne que le sous-ensemble des captures réellement partagées.

Une option plus ciblée, spécifique à `HTTPServer` et sans toucher au mécanisme général de fermeture : sérialiser l'INVOCATION du handler dans `handle_request` (`runtime/src/httpserver.rs`) avec un mutex par serveur autour du seul appel `f(h.env_ptr, req_handle)` — élimine la race par construction (deux handlers ne s'exécutent alors jamais en même temps), au prix de perdre le parallélisme RÉEL sur la logique métier (la lecture de la requête et l'envoi de la réponse, avant/après cet appel, resteraient parallèles). C'est un changement de philosophie : `HTTPServer` deviendrait "sûr par défaut" alors que `Thread` reste "rapide par défaut, sûr sur demande (Mutex)" — une incohérence de modèle entre les deux primitives de concurrence du langage, pas seulement un choix technique.

**Décision demandée** : garder l'option 1 (documentation + Mutex explicite, cohérent avec `Thread`) comme solution définitive, ou investir dans la sérialisation native malgré le coût de parallélisme et l'incohérence de modèle avec `Thread` ? Pas tranché ici — voir aussi que le bug de fermeture imbriquée découverte ci-dessus limite de toute façon l'utilisabilité de `withLock` dans un handler tant qu'il n'est pas corrigé, ce qui pèse sur cette décision.

## Priorité / Complexité

**Option 1 : ✅ Terminée.** Option 2 : **non tranchée**, ticket laissé ouvert en Priorité Haute jusqu'à décision — race condition réelle toujours possible pour un utilisateur qui ne suit pas la documentation (mitigée, pas éliminée par construction). **Complexité : Structurel** pour l'option 2, quelle que soit la variante retenue.

## Fichiers clés

`runtime/src/httpserver.rs`, `docs/builtins/HTTPServer.md` (fait), `examples/builtins/httpserver.oc`/`.sh` (fait), `examples/advanced/httpserver/`, `examples/advanced/mini_project/`, [langage-nested-closure-recapture](langage-nested-closure-recapture.md) (bug connexe découvert).
