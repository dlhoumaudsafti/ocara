#!/usr/bin/env bash
# Test de régression pour examples/builtins/httpserver_robustness.oc
# Usage : examples/builtins/httpserver_robustness.sh <binaire_compilé>
BIN=${1:?Usage: httpserver_robustness.sh <binaire>}
PORT=8093
URL="http://127.0.0.1:$PORT"
ROOT=/tmp/oc_httpserver_robustness

rm -rf "$ROOT"
mkdir -p "$ROOT"
head -c 3000000 /dev/urandom > "$ROOT/blob.bin"
echo accent > "$ROOT/é.txt"

"$BIN" 2>/dev/null &
SRV_PID=$!
for i in $(seq 1 20); do
    curl -s --max-time 1 "$URL/i/1" >/dev/null && break
    sleep 0.25
done

FAIL=0
expect() {
    if [ "$2" != "$3" ]; then
        echo "FAIL: $1 — attendu « $3 », reçu « $2 »" >&2
        FAIL=1
    fi
}

expect "exception dans un handler" "$(curl -s --max-time 3 -w '|%{http_code}' "$URL/boom")" "custom 500|500"
expect "serveur toujours actif" "$(curl -s --max-time 3 "$URL/i/5")" "id=5"
expect "injection CRLF" "$(curl -s --max-time 3 -D - -o /dev/null "$URL/echo?v=a%0d%0aX-Injected:%20yes" | grep -ci '^X-Injected')" "0"
expect "injection LF" "$(curl -s --max-time 3 -o /dev/null -w '%{http_code}' "$URL/echo?v=a%0aSet-Cookie:%20evil=1")" "500"
expect "en-tête valide" "$(curl -s --max-time 3 -D - -o /dev/null "$URL/echo?v=fine" | tr -d '\r' | grep '^X-Echo')" "X-Echo: fine"
expect "UTF-8 décodé" "$(curl -s --max-time 3 "$URL/s/h%C3%A9llo?q=caf%C3%A9+cr%C3%A8me")" "héllo|café crème"
expect "+ littéral dans un chemin" "$(curl -s --max-time 3 "$URL/s/a+b")" "a+b|"
expect "fragment hors query" "$(printf 'GET /s/x?q=1#frag HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n' | nc -q2 127.0.0.1 $PORT | tail -1)" "x|1"
if ! curl -s --max-time 5 "$URL/blob.bin" | cmp -s - "$ROOT/blob.bin"; then
    echo "FAIL: fichier statique binaire corrompu" >&2
    FAIL=1
fi
expect "fichier statique au nom encodé" "$(curl -s --max-time 3 "$URL/%C3%A9.txt")" "accent"
expect "paramètre int encodé" "$(curl -s --max-time 3 "$URL/i/%31")" "id=1"
expect "cookies répétés" "$(curl -s --max-time 3 -H 'Cookie: a=1' -H 'Cookie: b=2' "$URL/cookie")" "a=1; b=2"
expect "HEAD sur une route GET" "$(curl -s --max-time 3 -I -o /dev/null -w '%{http_code}' "$URL/i/5")" "200"
expect "405 et Allow" "$(curl -s --max-time 3 -X POST -D - -o /dev/null "$URL/i/5" | tr -d '\r' | grep -E '^(HTTP|Allow)' | tr '\n' ' ')" "HTTP/1.1 405 Method Not Allowed Allow: GET "
expect "float nan refusé" "$(curl -s --max-time 3 -o /dev/null -w '%{http_code}' "$URL/f/nan")" "404"
expect "en-tête non ASCII" "$(printf 'GET /i/5 HTTP/1.1\r\nHost: x\r\nX-Test: \xc3\xa9\r\nConnection: close\r\n\r\n' | nc -q2 127.0.0.1 $PORT | head -1 | tr -d '\r')" "HTTP/1.1 400 Bad Request"

# Téléversements lents (3 × workers) : une requête normale passe quand même.
slow_pids=""
for i in $(seq 1 6); do
    { printf 'POST /up HTTP/1.1\r\nHost: x\r\nContent-Length: 100000\r\n\r\nabc'; sleep 3; } | nc 127.0.0.1 $PORT >/dev/null 2>&1 &
    slow_pids="$slow_pids $!"
done
sleep 0.5
expect "requête pendant des envois lents" "$(curl -s --max-time 2 "$URL/i/7")" "id=7"
kill $slow_pids 2>/dev/null
wait $slow_pids 2>/dev/null

kill "$SRV_PID" 2>/dev/null
wait "$SRV_PID" 2>/dev/null
rm -rf "$ROOT"
exit $FAIL
