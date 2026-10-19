# HTTPServer : arrêt sur exception, injection d'en-têtes, déni de service, encodages

Audit du 2026-10-19. Reproduit sur le binaire courant (ports de test, `curl`
et scripts Python).

## Constat — sécurité et disponibilité

1. **Une exception non rattrapée dans un handler arrête tout le serveur.**
   `raise use MyErr("boom", 3)` dans une route : boîte « UNHANDLED
   EXCEPTION », puis le processus s'arrête. Attendu : une réponse 500 (ou
   `routeError(500)`). `call_handler_locked` (`runtime/src/httpserver.rs`
   ~l.846) n'utilise pas `run_closure_catching`.
2. **Injection CRLF dans `respondHeader`.** `req.respondHeader("X-Echo",
   req.query("v"))` avec `?v=a%0d%0aX-Injected:%20yes` produit un vrai
   en-tête ; `%0aSet-Cookie:%20evil=1` injecte un cookie. Le client
   (`HTTPRequest.setHeader`) refuse correctement.
3. **Déni de service par téléversements lents.** N connexions (N = `workers`)
   qui annoncent `Content-Length: 100000` et n'envoient que 3 octets bloquent
   toute autre requête indéfiniment (8 workers : 7 passent, 8 bloquent). Le
   corps est lu dans le worker sans délai (`handle_request` ~l.878-887).

## Constat — comportement incorrect

4. **Décodage URL : UTF-8 en pourcentage → mojibake.** `/s/h%C3%A9llo` →
   `hÃ©llo` (chemin, query, corps urlencoded, session). `url_decode`
   (~l.425-447) fait `n as char` octet par octet.
5. **Fichiers statiques binaires corrompus** : un `.bin` de 3 000 000 octets
   est servi en 5 438 269 octets (`String::from_utf8_lossy`, ~l.822). Casse
   `.png`, `.jpg`, `.woff`.
6. Routage et en-têtes :
   - `+` dans un segment de chemin décodé en espace ;
   - fichier statique au nom encodé introuvable (`/%C3%A9.txt` → 404) ;
     `/i/%31` ne correspond pas à `<id:int>` ;
   - en-tête non ASCII (`X-Test: é`) ou chemin non UTF-8 : connexion fermée
     sans réponse ;
   - deux en-têtes `Cookie` : seul le dernier est lu ;
   - `HEAD` sur une route GET → 404 ; autre méthode sur une route existante
     → 404 au lieu de 405 ;
   - `<x:float>` accepte `nan` et `inf` ;
   - `#frag` reste dans la query (non confirmé).

## Priorité / Complexité

**Haute** (sécurité, disponibilité) — **Légère** à **Structurel** (délais de
lecture).

## Fichiers clés

`runtime/src/httpserver.rs`, `runtime/src/httpsession.rs`,
`docs/builtins/HTTPServer.md`.
