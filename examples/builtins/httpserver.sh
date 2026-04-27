#!/usr/bin/env bash
# Test de régression pour examples/builtins/httpserver.oc
# Usage : examples/builtins/httpserver.sh <binaire_compilé>
BIN=${1:?Usage: httpserver.sh <binaire>}
PORT=8080

"$BIN" &
SRV_PID=$!

# Attendre que le port soit ouvert (max 5s)
for i in $(seq 1 10); do
    curl -s --max-time 1 "http://localhost:$PORT/" >/dev/null 2>/dev/null && break
    sleep 0.5
done

FAIL=0

# GET / → doit retourner quelque chose
resp=$(curl -s --max-time 3 "http://localhost:$PORT/")
if [ -z "$resp" ]; then
    echo "FAIL: GET / n'a rien retourné" >&2
    FAIL=1
fi

# GET /?name=Alice → doit contenir "Alice"
resp=$(curl -s --max-time 3 "http://localhost:$PORT/?name=Alice")
if ! echo "$resp" | grep -q "Alice"; then
    echo "FAIL: GET /?name=Alice ne contient pas 'Alice' (reçu: $resp)" >&2
    FAIL=1
fi

# POST /echo → doit renvoyer le corps
resp=$(curl -s --max-time 3 -X POST -d "bonjour" "http://localhost:$PORT/echo")
if ! echo "$resp" | grep -q "bonjour"; then
    echo "FAIL: POST /echo ne renvoie pas le corps (reçu: $resp)" >&2
    FAIL=1
fi

kill "$SRV_PID" 2>/dev/null
wait "$SRV_PID" 2>/dev/null
exit $FAIL
