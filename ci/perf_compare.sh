#!/usr/bin/env bash

# ══════════════════════════════════════════════════════════════════════════════
# Performances comparées à Python, PHP, Perl, C, C++ et Rust (make perf)
# ══════════════════════════════════════════════════════════════════════════════
# Compile chaque ci/perf/<nom>.oc, l'exécute avec la taille indiquée et le
# compare au même algorithme dans chaque langage (ci/perf/<nom>.py, .php,
# .pl, .c, .cpp, .rs — voir ci/perf_langs.sh) : minimum de 5 exécutions.
# Échoue si une mesure Ocara est plus lente qu'un langage interprété
# (Python, PHP, Perl) — un langage compilé ne doit jamais l'être (voir
# docs/benchmarking.md) ; C, C++ et Rust sont affichés en référence. « — » :
# mesure sautée pour ce langage. Hors de `make regression` : les temps
# dépendent de la charge de la machine.
#   ./ci/perf_compare.sh          → toutes les mesures
#   ./ci/perf_compare.sh split    → une seule
# ══════════════════════════════════════════════════════════════════════════════

OCARA="./target/release/ocara"
TMP_DIR=$(mktemp -d)
GREEN='\033[0;32m'
RED='\033[0;31m'
RESET='\033[0m'
trap 'rm -rf "$TMP_DIR"' EXIT
source "$(dirname "$0")/perf_langs.sh"

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
    if ! $OCARA "ci/perf/$name.oc" -o "$TMP_DIR/$name" > "$TMP_DIR/$name.log" 2>&1; then
        echo -e "${RED}FAIL [compile] $name${RESET}"; sed 's/^/    /' "$TMP_DIR/$name.log"; status=1; continue
    fi
    ocara=$(best "$TMP_DIR/$name" "$size")
    line="Ocara ${ocara} s"
    slower=()
    for lang in "${LANGS[@]}"; do
        if ! lang_cmd "$lang" "ci/perf/$name" "$TMP_DIR"; then
            line+=" | $(lang_label "$lang") —"; continue
        fi
        t=$(best "${CMD[@]}" "$size")
        line+=" | $(lang_label "$lang") $t"
        if is_interpreted "$lang" && ! awk -v o="$ocara" -v p="$t" 'BEGIN { exit !(o <= p) }'; then
            slower+=("$(lang_label "$lang")")
        fi
    done
    if [ ${#slower[@]} -eq 0 ]; then
        echo -e "${GREEN}OK   $name${RESET} (n=$size) : $line"
    else
        echo -e "${RED}FAIL $name${RESET} (n=$size) : $line — plus lent que ${slower[*]}"
        status=1
    fi
done
exit $status
