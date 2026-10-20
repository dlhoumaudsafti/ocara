#!/usr/bin/env bash

# ══════════════════════════════════════════════════════════════════════════════
# Croissance mémoire bornée en multi-thread
# ══════════════════════════════════════════════════════════════════════════════
# examples/34_concurrent_cycles.oc crée environ 700 000 cycles depuis
# plusieurs threads à la fois. Sans collecte concurrente des cycles, sa
# mémoire résidente dépasse 200 Mo ; la passe échoue au-delà de MAX_RSS_KB.
# Voir docs/roadmap.d/memoire-refcount.md (« Collecte concurrente »).
# ══════════════════════════════════════════════════════════════════════════════

OCARA="./target/release/ocara"
SRC="examples/34_concurrent_cycles.oc"
MAX_RSS_KB=120000
# Délais (s) : un programme bloqué (interblocage, boucle) fait échouer la
# passe au lieu de bloquer la CI.
COMPILE_TIMEOUT_S=300
RUN_TIMEOUT_S=120
BIN=$(mktemp)
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
RESET='\033[0m'

trap 'rm -f "$BIN"' EXIT

echo "══════════════════════════════════════════════"
echo " Mémoire bornée en multi-thread"
echo "══════════════════════════════════════════════"

if [ ! -x /usr/bin/time ]; then
    echo -e "${YELLOW}SKIP : /usr/bin/time absent, mesure impossible${RESET}"
    exit 0
fi

if ! timeout -k 10 "$COMPILE_TIMEOUT_S" $OCARA "$SRC" -o "$BIN" > /dev/null; then
    echo -e "${RED}FAIL [compile] $SRC${RESET}"
    exit 1
fi

# `timeout` tue tout son groupe de processus : /usr/bin/time ET le programme.
rss_file=$(mktemp)
timeout -k 10 "$RUN_TIMEOUT_S" /usr/bin/time -f "%M" -o "$rss_file" "$BIN" > /dev/null
rc=$?
rss=$(grep -E '^[0-9]+$' "$rss_file")
rm -f "$rss_file"

if [ $rc -eq 124 ] || [ $rc -eq 137 ]; then
    echo -e "${RED}FAIL [run, délai de ${RUN_TIMEOUT_S} s dépassé] $SRC${RESET}"
    exit 1
fi
if [ $rc -ne 0 ]; then
    echo -e "${RED}FAIL [run, code $rc] $SRC${RESET}"
    exit 1
fi
if [ -z "$rss" ]; then
    echo -e "${RED}FAIL : pic mémoire illisible (/usr/bin/time)${RESET}"
    exit 1
fi
if [ "$rss" -gt "$MAX_RSS_KB" ]; then
    echo -e "${RED}FAIL : mémoire résidente maximale ${rss} Ko > ${MAX_RSS_KB} Ko${RESET}"
    exit 1
fi
echo -e "${GREEN}OK   mémoire résidente maximale ${rss} Ko (borne ${MAX_RSS_KB} Ko)${RESET}"
