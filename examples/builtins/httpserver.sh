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

# GET /hits en vraie concurrence → le compteur ne doit perdre AUCUNE
# incrémentation, sans Mutex explicite : HTTPServer sérialise nativement
# l'invocation des handlers (docs/roadmap.d/runtime-httpserver-race-
# condition.md, option 2). N requêtes lancées simultanément (pas
# séquentiellement) sur un serveur à 4 workers : sans cette sérialisation,
# deux requêtes qui liraient hitCount avant que l'une n'ait fini de
# l'incrémenter perdraient un point.
N=30
curl_pids=""
for i in $(seq 1 "$N"); do
    curl -s --max-time 5 "http://localhost:$PORT/hits" >/dev/null 2>&1 &
    curl_pids="$curl_pids $!"
done
# `wait` SANS argument attendrait aussi $SRV_PID (le serveur, jamais
# terminé tant qu'on ne le kill pas) — attendre explicitement seulement les
# PID des curl lancés ci-dessus.
wait $curl_pids
final=$(curl -s --max-time 5 "http://localhost:$PORT/hits")
expected=$((N + 1))
if [ "$final" != "$expected" ]; then
    echo "FAIL: GET /hits concurrent — attendu $expected après $N requêtes simultanées + 1 vérification, reçu $final (incrémentation perdue = race condition)" >&2
    FAIL=1
fi

kill "$SRV_PID" 2>/dev/null
wait "$SRV_PID" 2>/dev/null
exit $FAIL
