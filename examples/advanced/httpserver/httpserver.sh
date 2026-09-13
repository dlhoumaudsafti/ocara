#!/usr/bin/env bash
# Test de régression pour examples/advanced/httpserver/main.oc
# Usage : httpserver.sh <binaire_compilé>
BIN=${1:?Usage: httpserver.sh <binaire>}
PORT=8080

"$BIN" &
SRV_PID=$!

# Attendre que le port soit ouvert (max 5s) — /about est utilisé pour le
# sondage plutôt que / : / charge deux statistiques en parallèle via
# async/resolve (Thread::sleep 3s + 5s), donc répond volontairement ~5s
# après le démarrage effectif du serveur.
for i in $(seq 1 10); do
    curl -s --max-time 1 "http://localhost:$PORT/about" >/dev/null 2>/dev/null && break
    sleep 0.5
done

FAIL=0

# GET / → page d'accueil (async/resolve), attendre son délai volontaire (~5s)
resp=$(curl -s --max-time 8 "http://localhost:$PORT/")
if ! echo "$resp" | grep -q "Bienvenue sur Ocara"; then
    echo "FAIL: GET / ne contient pas 'Bienvenue sur Ocara' (reçu: $resp)" >&2
    FAIL=1
fi

# GET /about → page statique (renderCached)
resp=$(curl -s --max-time 3 "http://localhost:$PORT/about")
if ! echo "$resp" | grep -q "A propos d'Ocara"; then
    echo "FAIL: GET /about ne contient pas le contenu attendu (reçu: $resp)" >&2
    FAIL=1
fi

# GET /contact → page statique (renderCached)
resp=$(curl -s --max-time 3 "http://localhost:$PORT/contact")
if [ -z "$resp" ]; then
    echo "FAIL: GET /contact n'a rien retourné" >&2
    FAIL=1
fi

# GET /version → API JSON
resp=$(curl -s --max-time 3 "http://localhost:$PORT/version")
if ! echo "$resp" | grep -q '"status":"ok"'; then
    echo "FAIL: GET /version ne contient pas 'status:ok' (reçu: $resp)" >&2
    FAIL=1
fi

# GET /users → API JSON (liste)
resp=$(curl -s --max-time 3 "http://localhost:$PORT/users")
if ! echo "$resp" | grep -q "Alice"; then
    echo "FAIL: GET /users ne contient pas 'Alice' (reçu: $resp)" >&2
    FAIL=1
fi

# GET /unknown → page 404 personnalisée
status=$(curl -s --max-time 3 -o /dev/null -w "%{http_code}" "http://localhost:$PORT/unknown")
if [ "$status" != "404" ]; then
    echo "FAIL: GET /unknown devait renvoyer 404 (reçu: $status)" >&2
    FAIL=1
fi

kill "$SRV_PID" 2>/dev/null
wait "$SRV_PID" 2>/dev/null
exit $FAIL
