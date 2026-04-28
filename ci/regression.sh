#!/usr/bin/env bash
set -e

# ══════════════════════════════════════════════════════════════════════════════
# Script de régression Ocara
# ══════════════════════════════════════════════════════════════════════════════
# Ce script compile et exécute les tests de régression pour le compilateur Ocara.
# Il peut être exécuté avec ou sans argument :
#   - Sans argument : exécute tous les tests
#   - Avec argument : exécute uniquement le test spécifié (ex: ./ci/regression.sh 30_variadic)
# ══════════════════════════════════════════════════════════════════════════════

OCARA="./target/release/ocara"
TMP="/tmp/oc_regression"
GREEN='\033[0;32m'
RED='\033[0;31m'
RESET='\033[0m'

# ── Vérifier que le compilateur existe ───────────────────────────────────────
if [ ! -x "$OCARA" ]; then
    echo -e "${RED}Erreur : $OCARA n'existe pas ou n'est pas exécutable${RESET}"
    echo "Exécutez d'abord : make build"
    exit 1
fi

# ── Fonction pour compiler et exécuter un test ────────────────────────────────
run_test() {
    local src="$1"
    local name="$2"
    local input="${3:-}"
    
    # Cas spécial : 21_errors doit échouer à la compilation
    if [ "$name" = "21_errors" ]; then
        $OCARA "$src" --check > /dev/null 2>&1
        rc=$?
        if [ $rc -eq 0 ]; then
            echo -e "${RED}FAIL [check devait échouer] $name${RESET}"
            return 1
        else
            echo -e "${GREEN}OK   $name${RESET}"
            return 0
        fi
    fi
    
    # Compilation
    $OCARA "$src" -o "$TMP" 2>&1
    rc=$?
    if [ $rc -ne 0 ]; then
        echo -e "${RED}FAIL [compile] $name${RESET}"
        rm -f "$TMP"
        return 1
    fi
    
    echo "compilation réussie → $TMP"
    
    # Exécution avec gestion des cas spéciaux
    case "$name" in
        03_builtins)
            echo -e "david\n45" | "$TMP"
            ;;
        io)
            printf 'Alice\nParis\n21\n3.14\ntrue\nrust,ocara,web\nlang=fr,theme=dark\n' | "$TMP"
            ;;
        http)
            "$TMP" > /dev/null
            ;;
        httpserver)
            examples/builtins/httpserver.sh "$TMP"
            ;;
        *)
            "$TMP"
            ;;
    esac
    
    rc=$?
    if [ $rc -ne 0 ]; then
        echo -e "${RED}FAIL [run] $name${RESET}"
        rm -f "$TMP"
        return 1
    fi
    
    echo -e "${GREEN}OK   $name${RESET}"
    rm -f "$TMP"
    return 0
}

# ── Exécution d'un seul test (si argument fourni) ─────────────────────────────
if [ $# -eq 1 ]; then
    TARGET="$1"
    src="examples/${TARGET}.oc"
    
    if [ ! -f "$src" ]; then
        echo -e "${RED}Erreur : $src n'existe pas${RESET}"
        exit 1
    fi
    
    echo "══════════════════════════════════════════════"
    echo " Régression $src"
    echo "══════════════════════════════════════════════"
    
    run_test "$src" "$TARGET"
    exit $?
fi

# ── Exécution de tous les tests ──────────────────────────────────────────────
fail=0
failed=""

# Tests NN_*.oc
echo "══════════════════════════════════════════════"
echo " Régression examples/NN_*.oc"
echo "══════════════════════════════════════════════"

for src in examples/[0-9][0-9]_*.oc; do
    if [ ! -f "$src" ]; then
        continue
    fi
    
    name=$(basename "$src" .oc)
    
    if ! run_test "$src" "$name"; then
        fail=1
        failed="$failed $name"
    fi
done

echo ""

# Test project/main.oc
echo "══════════════════════════════════════════════"
echo " Régression examples/project/main.oc"
echo "══════════════════════════════════════════════"

if ! run_test "examples/project/main.oc" "project/main"; then
    fail=1
    failed="$failed project/main"
fi

echo ""

# Tests builtins/*.oc
echo "══════════════════════════════════════════════"
echo " Régression examples/builtins/*.oc"
echo "══════════════════════════════════════════════"

for src in examples/builtins/*.oc; do
    if [ ! -f "$src" ]; then
        continue
    fi
    
    name=$(basename "$src" .oc)
    
    if ! run_test "$src" "$name"; then
        fail=1
        failed="$failed builtins/$name"
    fi
done

rm -f "$TMP"

# ── Résultat final ────────────────────────────────────────────────────────────
echo ""
echo "══════════════════════════════════════════════"

if [ $fail -eq 0 ]; then
    echo -e "${GREEN}Tous les tests ont réussi.${RESET}"
    echo "══════════════════════════════════════════════"
    exit 0
else
    echo -e "${RED}Échecs :${RESET}"
    for f in $failed; do
        echo -e "  ${RED}✗ $f${RESET}"
    done
    echo ""
    echo -e "${RED}Des tests ont échoué.${RESET}"
    echo "══════════════════════════════════════════════"
    exit 1
fi
