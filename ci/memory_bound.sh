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

if ! $OCARA "$SRC" -o "$BIN" > /dev/null; then
    echo -e "${RED}FAIL [compile] $SRC${RESET}"
    exit 1
fi

rss_file=$(mktemp)
/usr/bin/time -f "%M" -o "$rss_file" "$BIN" > /dev/null
rc=$?
rss=$(cat "$rss_file")
rm -f "$rss_file"

if [ $rc -ne 0 ]; then
    echo -e "${RED}FAIL [run, code $rc] $SRC${RESET}"
    exit 1
fi
if [ "$rss" -gt "$MAX_RSS_KB" ]; then
    echo -e "${RED}FAIL : mémoire résidente maximale ${rss} Ko > ${MAX_RSS_KB} Ko${RESET}"
    exit 1
fi
echo -e "${GREEN}OK   mémoire résidente maximale ${rss} Ko (borne ${MAX_RSS_KB} Ko)${RESET}"
