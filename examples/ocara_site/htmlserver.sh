#!/usr/bin/env bash
# Test de régression pour examples/builtins/html/main.oc
# Usage : examples/builtins/html/htmlserver.sh <binaire_compilé>

BIN=${1:?Usage: htmlserver.sh <binaire>}
PORT=3000

"$BIN" &
SRV_PID=$!

# Attendre que le port soit ouvert (max 5s)
for i in $(seq 1 10); do
    curl -s --max-time 1 "http://localhost:$PORT/" >/dev/null && break
    sleep 0.5
done

FAIL=0

# GET / → doit contenir les stats dynamiques (async/resolve)
resp=$(curl -s --max-time 5 "http://localhost:$PORT/")
if ! echo "$resp" | grep -q "utilisateurs"; then
    echo "FAIL: GET / ne contient pas les stats (utilisateurs)" >&2
    FAIL=1
fi
if ! echo "$resp" | grep -q "Ocara"; then
    echo "FAIL: GET / ne contient pas 'Ocara'" >&2
    FAIL=1
fi

# GET /about → doit contenir "propos" (render_cached)
resp=$(curl -s --max-time 5 "http://localhost:$PORT/about")
if ! echo "$resp" | grep -q "propos"; then
    echo "FAIL: GET /about ne contient pas 'propos'" >&2
    FAIL=1
fi

# GET /contact → doit contenir "contacter" (render_cached)
resp=$(curl -s --max-time 5 "http://localhost:$PORT/contact")
if ! echo "$resp" | grep -q "contacter"; then
    echo "FAIL: GET /contact ne contient pas 'contacter'" >&2
    FAIL=1
fi

# Second appel /about → teste le hit de cache
resp=$(curl -s --max-time 5 "http://localhost:$PORT/about")
if ! echo "$resp" | grep -q "propos"; then
    echo "FAIL: GET /about (cache hit) ne contient pas 'propos'" >&2
    FAIL=1
fi

# La nav-bar doit être présente sur chaque page
resp=$(curl -s --max-time 5 "http://localhost:$PORT/")
if ! echo "$resp" | grep -q "nav-link"; then
    echo "FAIL: GET / ne contient pas la nav-bar" >&2
    FAIL=1
fi

kill "$SRV_PID" 2>/dev/null
wait "$SRV_PID" 2>/dev/null
exit $FAIL
