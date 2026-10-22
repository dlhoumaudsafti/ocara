#!/usr/bin/env bash

# ══════════════════════════════════════════════════════════════════════════════
# Mesures hors calcul comparées à Python, PHP, Perl, C, C++ et Rust
# (make perf-extra)
# ══════════════════════════════════════════════════════════════════════════════
#   - démarrage : programme vide de chaque langage (ci/perf/extra/empty.*,
#     50 lancements) ;
#   - mémoire : résidente maximale de chaque mesure de ci/perf/benches.txt ;
#   - serveur HTTP : requêtes par seconde (ci/perf/extra/http_load.py, même
#     client pour chaque serveur ; C, C++ et Rust n'en ont pas dans leur
#     bibliothèque standard).
# Échoue si Ocara démarre plus lentement, consomme plus de mémoire ou sert
# moins de requêtes qu'un langage interprété (Python, PHP, Perl) ; C, C++ et
# Rust sont affichés en référence (voir ci/perf_langs.sh). Hors de
# `make regression` : les mesures dépendent de la charge de la machine.
# ══════════════════════════════════════════════════════════════════════════════

OCARA="./target/release/ocara"
TMP_DIR=$(mktemp -d)
GREEN='\033[0;32m'
RED='\033[0;31m'
RESET='\033[0m'
HTTP_PORT=18431
trap 'rm -rf "$TMP_DIR"' EXIT
source "$(dirname "$0")/perf_langs.sh"

status=0

# compare <libellé> <unité> <plus petit|plus grand> <valeur Ocara> <base> <mesure…>
# Mesure chaque langage avec la commande `mesure` suivie de CMD, affiche une
# ligne et échoue si un langage interprété fait mieux qu'Ocara.
compare() {
    local label=$1 unit=$2 better=$3 ocara=$4 base=$5 measure=$6 mode=${7:-}
    local line="Ocara $ocara" slower=() lang value ok
    for lang in "${LANGS[@]}"; do
        if ! lang_cmd "$lang" "$base" "$TMP_DIR" "$mode"; then
            line+=" | $(lang_label "$lang") —"; continue
        fi
        value=$($measure "${CMD[@]}")
        line+=" | $(lang_label "$lang") $value"
        if [ "$better" = "plus petit" ]; then
            ok=$(awk -v o="$ocara" -v p="$value" 'BEGIN { print (o <= p) }')
        else
            ok=$(awk -v o="$ocara" -v p="$value" 'BEGIN { print (o >= p) }')
        fi
        if is_interpreted "$lang" && [ "$ok" != "1" ]; then
            slower+=("$(lang_label "$lang")")
        fi
    done
    if [ ${#slower[@]} -eq 0 ]; then
        echo -e "${GREEN}OK   $label${RESET} ($unit) : $line"
    else
        echo -e "${RED}FAIL $label${RESET} ($unit) : $line — moins bon que ${slower[*]}"
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

SIZE=0
rss() {
    { /usr/bin/time -f "%M" "$@" "$SIZE" > /dev/null; } 2>&1 | tail -1
}

# Requêtes par seconde du serveur lancé par la commande (port en dernier
# argument), arrêté avec ses processus fils.
serve() {
    setsid "$@" "$HTTP_PORT" > /dev/null 2>&1 &
    local pid=$! rate
    rate=$(timeout 120 python3 ci/perf/extra/http_load.py "$HTTP_PORT" 4 2000)
    kill -- -"$pid" 2> /dev/null
    wait "$pid" 2> /dev/null
    echo "${rate:-0}"
}

echo "── Démarrage"
$OCARA ci/perf/extra/empty.oc -o "$TMP_DIR/empty" > /dev/null 2>&1 || { echo -e "${RED}FAIL [compile] empty${RESET}"; exit 1; }
compare "démarrage" ms "plus petit" "$(launch_ms "$TMP_DIR/empty")" ci/perf/extra/empty launch_ms

echo "── Mémoire résidente maximale"
while read -r name size; do
    $OCARA "ci/perf/$name.oc" -o "$TMP_DIR/$name" > /dev/null 2>&1 || { echo -e "${RED}FAIL [compile] $name${RESET}"; status=1; continue; }
    SIZE=$size
    compare "mémoire $name (n=$size)" Kio "plus petit" "$(rss "$TMP_DIR/$name")" "ci/perf/$name" rss
done < <(grep -v '^#' ci/perf/benches.txt)

echo "── Serveur HTTP"
$OCARA ci/perf/extra/http_server.oc -o "$TMP_DIR/http_server" > /dev/null 2>&1 || { echo -e "${RED}FAIL [compile] http_server${RESET}"; exit 1; }
ocara_rate=$(serve "$TMP_DIR/http_server")
if [ "$ocara_rate" = "0" ]; then
    echo -e "${RED}FAIL serveur HTTP${RESET} : le serveur Ocara n'a pas répondu"
    status=1
else
    compare "serveur HTTP" "req/s" "plus grand" "$ocara_rate" ci/perf/extra/http_server serve build
fi

exit $status
