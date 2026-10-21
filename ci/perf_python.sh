#!/usr/bin/env bash

# ══════════════════════════════════════════════════════════════════════════════
# Performances comparées à Python (make perf)
# ══════════════════════════════════════════════════════════════════════════════
# Compile chaque ci/perf/<nom>.oc, l'exécute avec la taille indiquée et la
# compare au même algorithme en Python (ci/perf/<nom>.py) : minimum de 5
# exécutions de chaque côté. Échoue si une mesure Ocara est plus lente que
# Python — un langage compilé ne doit jamais l'être (voir
# docs/benchmarking.md). Hors de `make regression` : les temps dépendent de
# la charge de la machine.
#   ./ci/perf_python.sh          → toutes les mesures
#   ./ci/perf_python.sh split    → une seule
# ══════════════════════════════════════════════════════════════════════════════

OCARA="./target/release/ocara"
TMP_DIR=$(mktemp -d)
GREEN='\033[0;32m'
RED='\033[0;31m'
RESET='\033[0m'
trap 'rm -rf "$TMP_DIR"' EXIT

# nom taille, une mesure par ligne
mapfile -t BENCHES < <(grep -v '^#' ci/perf/benches.txt)

best() {
    local m=999 t
    for _ in 1 2 3 4 5; do
        t=$( { /usr/bin/time -f "%e" "$@" > /dev/null; } 2>&1 )
        m=$(awk -v a="$t" -v b="$m" 'BEGIN { print (a < b) ? a : b }')
    done
    echo "$m"
}

ONLY="${1:-}"
status=0
for spec in "${BENCHES[@]}"; do
    set -- $spec
    name=$1; size=$2
    [ -n "$ONLY" ] && [ "$ONLY" != "$name" ] && continue
    # Module Python absent (PyYAML…) : rien à comparer.
    if ! python3 "ci/perf/$name.py" 1 > /dev/null 2>&1; then
        echo "SKIP $name : ci/perf/$name.py ne s'exécute pas (module Python manquant ?)"; continue
    fi
    if ! $OCARA "ci/perf/$name.oc" -o "$TMP_DIR/$name" > "$TMP_DIR/$name.log" 2>&1; then
        echo -e "${RED}FAIL [compile] $name${RESET}"; sed 's/^/    /' "$TMP_DIR/$name.log"; status=1; continue
    fi
    ocara=$(best "$TMP_DIR/$name" "$size")
    python=$(best python3 "ci/perf/$name.py" "$size")
    if awk -v o="$ocara" -v p="$python" 'BEGIN { exit !(o <= p) }'; then
        echo -e "${GREEN}OK   $name${RESET} (n=$size) : Ocara ${ocara} s, Python ${python} s"
    else
        echo -e "${RED}FAIL $name${RESET} (n=$size) : Ocara ${ocara} s, Python ${python} s"
        status=1
    fi
done
exit $status
