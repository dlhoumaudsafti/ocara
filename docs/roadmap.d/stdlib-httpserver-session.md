# Nouvelle classe builtin `HTTPServerSession`

## Proposition

Une classe builtin pour l'état côté serveur, complément naturel de
`HTTPServerRequest` (voir [stdlib-httpserver-request-object](stdlib-httpserver-request-object.md)) :

- **Session par utilisateur** : `set(key:string, value:mixed)` /
  `get(key:string): mixed` / `has(key:string): bool` — des données attachées
  à UN visiteur précis, qui doivent survivre entre plusieurs requêtes de ce
  même visiteur (ex. un panier, un utilisateur connecté). `has()` permet de
  savoir si une clé existe/a déjà été initialisée sans avoir à interpréter
  une éventuelle valeur `null` légitime comme "absente" (`get()` sur une clé
  jamais posée et `get()` sur une clé explicitement mise à `null` seraient
  sinon indistinguables).
- **État global** : `setGlobal(key:string, value:mixed)` /
  `getGlobal(key:string): mixed` / `hasGlobal(key:string): bool` — un
  magasin clé/valeur unique, partagé par TOUTES les sessions/requêtes (ex.
  un cache applicatif, un compteur global), sans notion d'utilisateur — même
  distinction `hasGlobal()`/`getGlobal()` que ci-dessus.

## Ce qu'il faut trancher avant d'implémenter

- **Comment obtenir une instance liée à la requête courante ?** Le nom
  `HTTPServerSession` suggère une classe séparée de `HTTPServerRequest`,
  mais `set`/`get` (scope session) doivent forcément savoir DE QUELLE
  session il s'agit — probablement `req.session(): HTTPServerSession` (une
  nouvelle méthode sur `HTTPServerRequest`, dans l'esprit de ce qui existe
  déjà) plutôt que des méthodes statiques sans contexte. À l'inverse,
  `setGlobal`/`getGlobal` n'ont besoin d'aucune session — probablement
  statiques (`HTTPServerSession::setGlobal(...)`), à trancher.
- **Identification de session** : le mécanisme standard est un cookie
  (identifiant de session généré à la première visite, renvoyé au client,
  relu à chaque requête suivante). Aucun support cookie n'existe aujourd'hui
  dans `ocara.HTTPServer`/`HTTPServerRequest` (ni lecture du header `Cookie`,
  ni écriture de `Set-Cookie`) — prérequis probable de ce ticket, pas
  seulement une conséquence.
- **Stockage** : en mémoire, par processus (comme les autres simulations déjà
  en place dans ce projet, ex. Tauri `listen`/`emit`, voir
  [builtins-tauri](builtins-tauri.md)) — pas de persistance entre redémarrages
  pour une première version. Une session store qui ne grandit jamais ne se
  vide jamais est un vrai souci à terme (expiration/nettoyage) — probablement
  hors périmètre d'une v1, mais à documenter comme limitation connue plutôt
  que découvert plus tard.
- **Concurrence** : `HTTPServer` a déjà un historique de race condition
  (voir [runtime-httpserver-race-condition](runtime-httpserver-race-condition.md),
  clos) — un magasin partagé entre threads/workers a besoin d'une
  synchronisation correcte (probablement un `Mutex` interne, même famille que
  `ocara.Mutex` déjà utilisé ailleurs dans ce runtime), à concevoir dès le
  départ plutôt qu'à corriger après coup.

## Priorité / Complexité

Complexité non évaluée précisément avant d'avoir tranché les points
ci-dessus — probablement **Structurel** au minimum à cause du prérequis
cookie (rien n'existe aujourd'hui) et de la synchronisation concurrente.

## Fichiers clés

`runtime/src/httpserver.rs`, `src/builtins/httpserver.rs`,
`src/codegen/desc.d/httpserver.rs`, `docs/builtins/HTTPServer.md`.
