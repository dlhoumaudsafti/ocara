#!/usr/bin/env bash
# Test de régression pour examples/builtins/httpserver_session.oc
# Usage : httpserver_session.sh <binaire_compilé>
BIN=${1:?Usage: httpserver_session.sh <binaire>}
PORT=8208
JAR=$(mktemp)

"$BIN" &
SRV_PID=$!

for i in $(seq 1 10); do
    curl -s --max-time 1 "http://localhost:$PORT/me" >/dev/null 2>/dev/null && break
    sleep 0.5
done

FAIL=0
expect() {
    if [ "$2" != "$3" ]; then
        echo "FAIL: $1 (attendu: $2, reçu: $3)" >&2
        FAIL=1
    fi
}

expect "connexion" "logged in" "$(curl -s --max-time 3 -c "$JAR" -b "$JAR" "http://localhost:$PORT/login/ada")"
expect "session retrouvée" "user=ada visits=2" "$(curl -s --max-time 3 -c "$JAR" -b "$JAR" "http://localhost:$PORT/me")"
expect "sans cookie" "anonymous" "$(curl -s --max-time 3 "http://localhost:$PORT/me")"
expect "déconnexion" "logged out" "$(curl -s --max-time 3 -c "$JAR" -b "$JAR" "http://localhost:$PORT/logout")"
expect "après déconnexion" "anonymous" "$(curl -s --max-time 3 -c "$JAR" -b "$JAR" "http://localhost:$PORT/me")"

kill "$SRV_PID" 2>/dev/null
wait "$SRV_PID" 2>/dev/null
rm -f "$JAR"
exit $FAIL
