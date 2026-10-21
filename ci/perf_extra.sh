#!/usr/bin/env bash

# ══════════════════════════════════════════════════════════════════════════════
# Mesures hors calcul comparées à Python (make perf-extra)
# ══════════════════════════════════════════════════════════════════════════════
#   - démarrage : programme vide contre `python3 -c pass` (50 lancements) ;
#   - mémoire : résidente maximale de chaque mesure de ci/perf/benches.txt ;
#   - serveur HTTP : requêtes par seconde (ci/perf/extra/http_load.py, même
#     client pour les deux serveurs, connexions persistantes).
# Échoue si Ocara démarre plus lentement, consomme plus de mémoire ou sert
# moins de requêtes que Python. Hors de `make regression` : les mesures
# dépendent de la charge de la machine.
# ══════════════════════════════════════════════════════════════════════════════

OCARA="./target/release/ocara"
TMP_DIR=$(mktemp -d)
GREEN='\033[0;32m'
RED='\033[0;31m'
RESET='\033[0m'
HTTP_PORT=18431
trap 'rm -rf "$TMP_DIR"' EXIT

status=0

# verdict <libellé> <ocara> <python> <unité> <plus petit|plus grand>
verdict() {
    local ok
    if [ "$5" = "plus petit" ]; then
        ok=$(awk -v o="$2" -v p="$3" 'BEGIN { print (o <= p) }')
    else
        ok=$(awk -v o="$2" -v p="$3" 'BEGIN { print (o >= p) }')
    fi
    if [ "$ok" = "1" ]; then
        echo -e "${GREEN}OK   $1${RESET} : Ocara $2 $4, Python $3 $4"
    else
        echo -e "${RED}FAIL $1${RESET} : Ocara $2 $4, Python $3 $4"
        status=1
    fi
}

# Durée moyenne d'un lancement en millisecondes (50 lancements : /usr/bin/time
# ne mesure qu'au centième de seconde).
launch_ms() {
    local start end
    start=$(date +%s%N)
    for _ in $(seq 50); do "$@" > /dev/null; done
    end=$(date +%s%N)
    awk -v d="$((end - start))" 'BEGIN { printf "%.2f", d / 50 / 1000000 }'
}

rss() {
    { /usr/bin/time -f "%M" "$@" > /dev/null; } 2>&1 | tail -1
}

echo "── Démarrage"
$OCARA ci/perf/extra/empty.oc -o "$TMP_DIR/empty" > /dev/null 2>&1 || { echo -e "${RED}FAIL [compile] empty${RESET}"; exit 1; }
verdict "démarrage" "$(launch_ms "$TMP_DIR/empty")" "$(launch_ms python3 -c pass)" ms "plus petit"

echo "── Mémoire résidente maximale"
while read -r name size; do
    if ! python3 "ci/perf/$name.py" 1 > /dev/null 2>&1; then
        echo "SKIP $name : ci/perf/$name.py ne s'exécute pas"; continue
    fi
    $OCARA "ci/perf/$name.oc" -o "$TMP_DIR/$name" > /dev/null 2>&1 || { echo -e "${RED}FAIL [compile] $name${RESET}"; status=1; continue; }
    verdict "mémoire $name (n=$size)" "$(rss "$TMP_DIR/$name" "$size")" "$(rss python3 "ci/perf/$name.py" "$size")" Kio "plus petit"
done < <(grep -v '^#' ci/perf/benches.txt)

echo "── Serveur HTTP"
# serve <commande…> : requêtes par seconde du serveur lancé par la commande.
serve() {
    "$@" "$HTTP_PORT" > /dev/null 2>&1 &
    local pid=$!
    local rate
    rate=$(timeout 120 python3 ci/perf/extra/http_load.py "$HTTP_PORT" 4 2000)
    kill "$pid" 2> /dev/null
    wait "$pid" 2> /dev/null
    echo "${rate:-0}"
}
$OCARA ci/perf/extra/http_server.oc -o "$TMP_DIR/http_server" > /dev/null 2>&1 || { echo -e "${RED}FAIL [compile] http_server${RESET}"; exit 1; }
ocara_rate=$(serve "$TMP_DIR/http_server")
if [ "$ocara_rate" = "0" ]; then
    echo -e "${RED}FAIL serveur HTTP${RESET} : le serveur Ocara n'a pas répondu"
    status=1
else
    verdict "serveur HTTP" "$ocara_rate" "$(serve python3 ci/perf/extra/http_server.py)" "req/s" "plus grand"
fi

exit $status
