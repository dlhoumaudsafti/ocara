#!/usr/bin/env bash

# ══════════════════════════════════════════════════════════════════════════════
# Bilan des fuites Ocara (--leak-check)
# ══════════════════════════════════════════════════════════════════════════════
# Compile chaque examples/NN_*.oc avec `--leak-check`, l'exécute, et échoue si
# le bilan (stderr) rapporte une valeur non libérée.
#   - Sans argument : tous les exemples NN_*.oc
#   - Avec argument : uniquement l'exemple indiqué (ex: ./ci/leak_check.sh 07_loops)
#
# KNOWN_LEAKS : exemples dont la fuite est connue et suivie dans la roadmap.
# Leur fuite est signalée (KNOWN) sans faire échouer la passe ; un exemple de
# cette liste qui ne fuit plus fait échouer la passe, pour qu'on l'en retire.
# ══════════════════════════════════════════════════════════════════════════════

OCARA="./target/release/ocara"
TMP_DIR=$(mktemp -d)
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
RESET='\033[0m'

# Fuite connue et suivie dans la roadmap : y ajouter le nom de l'exemple.
KNOWN_LEAKS=()

trap 'rm -rf "$TMP_DIR"' EXIT

if [ ! -x "$OCARA" ]; then
    echo -e "${RED}Erreur : $OCARA n'existe pas ou n'est pas exécutable${RESET}"
    echo "Exécutez d'abord : make build"
    exit 1
fi

is_known_leak() {
    local name="$1" k
    for k in "${KNOWN_LEAKS[@]}"; do
        [ "$k" = "$name" ] && return 0
    done
    return 1
}

# ── Compile, exécute et analyse le bilan d'un exemple ────────────────────────
check_leaks() {
    local src="$1"
    local name
    name=$(basename "$src" .oc)
    local bin="$TMP_DIR/$name"
    local report="$TMP_DIR/$name.leaks"

    # Erreurs de compilation intentionnelles : rien à exécuter
    [ "$name" = "21_errors" ] && return 0

    if ! $OCARA "$src" --leak-check -o "$bin" > "$TMP_DIR/$name.compile" 2>&1; then
        echo -e "${RED}FAIL [compile] $name${RESET}"
        sed 's/^/    /' "$TMP_DIR/$name.compile"
        return 1
    fi

    case "$name" in
        03_builtins) echo -e "david\n45" | "$bin" > /dev/null 2> "$report" ;;
        *)           "$bin" < /dev/null > /dev/null 2> "$report" ;;
    esac
    local rc=$?
    if [ $rc -ne 0 ]; then
        echo -e "${RED}FAIL [run, code $rc] $name${RESET}"
        sed 's/^/    /' "$report"
        return 1
    fi

    if grep -q "── fuites : aucune" "$report"; then
        if is_known_leak "$name"; then
            echo -e "${RED}FAIL [ne fuit plus : retirer de KNOWN_LEAKS] $name${RESET}"
            return 1
        fi
        echo -e "${GREEN}OK   $name${RESET}"
        return 0
    fi

    if ! grep -q "── fuites :" "$report"; then
        echo -e "${RED}FAIL [aucun bilan des fuites] $name${RESET}"
        return 1
    fi

    if is_known_leak "$name"; then
        echo -e "${YELLOW}KNOWN $name${RESET}"
        return 0
    fi
    echo -e "${RED}FAIL [fuite] $name${RESET}"
    sed 's/^/    /' "$report"
    return 1
}

echo "══════════════════════════════════════════════"
echo " Bilan des fuites (--leak-check)"
echo "══════════════════════════════════════════════"

# ── Exécution d'un seul exemple ──────────────────────────────────────────────
if [ $# -eq 1 ]; then
    src="$1"
    [[ "$src" == *.oc ]] || src="${src}.oc"
    [[ "$src" == examples/* ]] || src="examples/$src"
    if [ ! -f "$src" ]; then
        echo -e "${RED}Erreur : $src n'existe pas${RESET}"
        exit 1
    fi
    if check_leaks "$src"; then
        exit 0
    fi
    exit 1
fi

# ── Tous les exemples NN_*.oc ────────────────────────────────────────────────
failed=""
for src in examples/[0-9][0-9]_*.oc; do
    [ -f "$src" ] || continue
    if ! check_leaks "$src"; then
        failed="$failed $(basename "$src" .oc)"
    fi
done

echo "══════════════════════════════════════════════"
if [ -z "$failed" ]; then
    echo -e "${GREEN}Aucune fuite nouvelle.${RESET}"
    echo "══════════════════════════════════════════════"
    exit 0
fi
echo -e "${RED}Échecs :${RESET}"
for f in $failed; do
    echo -e "  ${RED}✗ $f${RESET}"
done
echo "══════════════════════════════════════════════"
exit 1
