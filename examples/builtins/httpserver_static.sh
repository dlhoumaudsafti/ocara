#!/usr/bin/env bash
# Test de régression pour examples/builtins/httpserver_static.oc
# Usage : httpserver_static.sh <binaire_compilé>
BIN=${1:?Usage: httpserver_static.sh <binaire>}
PORT=8080

"$BIN" &
SRV_PID=$!

# Attendre que le port soit ouvert (max 5s)
for i in $(seq 1 10); do
    curl -s --max-time 1 "http://localhost:$PORT/api/status" >/dev/null 2>/dev/null && break
    sleep 0.5
done

FAIL=0

# GET /api/status → route dynamique JSON
resp=$(curl -s --max-time 3 "http://localhost:$PORT/api/status")
if ! echo "$resp" | grep -q '"status":"ok"'; then
    echo "FAIL: GET /api/status ne contient pas 'status:ok' (reçu: $resp)" >&2
    FAIL=1
fi

# POST /api/echo → doit renvoyer le corps
resp=$(curl -s --max-time 3 -X POST -d "bonjour" "http://localhost:$PORT/api/echo")
if ! echo "$resp" | grep -q "bonjour"; then
    echo "FAIL: POST /api/echo ne renvoie pas le corps (reçu: $resp)" >&2
    FAIL=1
fi

# GET /index.html → fichier statique (fallback, aucune route ne correspond)
resp=$(curl -s --max-time 3 "http://localhost:$PORT/index.html")
if [ -z "$resp" ]; then
    echo "FAIL: GET /index.html n'a rien retourné" >&2
    FAIL=1
fi

# GET /unknown.html → page 404 personnalisée
status=$(curl -s --max-time 3 -o /dev/null -w "%{http_code}" "http://localhost:$PORT/unknown.html")
if [ "$status" != "404" ]; then
    echo "FAIL: GET /unknown.html devait renvoyer 404 (reçu: $status)" >&2
    FAIL=1
fi

kill "$SRV_PID" 2>/dev/null
wait "$SRV_PID" 2>/dev/null
exit $FAIL
