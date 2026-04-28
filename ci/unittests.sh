#!/usr/bin/env bash
set -e

# ══════════════════════════════════════════════════════════════════════════════
# Script de tests unitaires Ocara
# ══════════════════════════════════════════════════════════════════════════════
# Ce script lance les tests unitaires avec ocaraunit.
# Il peut être exécuté avec ou sans argument :
#   - Sans argument : exécute les tests dans examples/project/tests
#   - Avec argument : exécute les tests dans le dossier spécifié
# ══════════════════════════════════════════════════════════════════════════════

OCARA="./target/release/ocara"
OCARAUNIT="./target/release/ocaraunit"
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
RESET='\033[0m'

# ── Vérifier que les outils existent ─────────────────────────────────────────
if [ ! -x "$OCARA" ]; then
    echo -e "${RED}Erreur : $OCARA n'existe pas ou n'est pas exécutable${RESET}"
    echo "Exécutez d'abord : make build"
    exit 1
fi

if [ ! -x "$OCARAUNIT" ]; then
    echo -e "${RED}Erreur : $OCARAUNIT n'existe pas ou n'est pas exécutable${RESET}"
    echo "Exécutez d'abord : make build-tools"
    exit 1
fi

# ── Fonction pour lancer les tests dans un dossier ────────────────────────────
run_tests() {
    local test_dir="$1"
    
    if [ ! -d "$test_dir" ]; then
        echo -e "${RED}Erreur : le dossier $test_dir n'existe pas${RESET}"
        return 1
    fi
    
    # Compter les fichiers de test
    local test_count=$(find "$test_dir" -name "*Test.oc" | wc -l)
    
    if [ "$test_count" -eq 0 ]; then
        echo -e "${YELLOW}Aucun fichier de test (*Test.oc) trouvé dans $test_dir${RESET}"
        return 0
    fi
    
    echo "══════════════════════════════════════════════"
    echo " Tests unitaires : $test_dir"
    echo " Fichiers de test : $test_count"
    echo "══════════════════════════════════════════════"
    
    # Sauvegarder le répertoire actuel
    local original_dir=$(pwd)
    
    # Résoudre le chemin absolu du dossier de tests
    local abs_test_dir=$(cd "$test_dir" && pwd)
    local parent_dir=$(dirname "$abs_test_dir")
    local tests_folder=$(basename "$abs_test_dir")
    
    # Calculer les chemins relatifs vers les binaires depuis le parent_dir
    local rel_ocara=$(realpath --relative-to="$parent_dir" "$OCARA")
    local rel_ocaraunit=$(realpath --relative-to="$parent_dir" "$OCARAUNIT")
    
    # Se déplacer dans le dossier parent et lancer ocaraunit
    cd "$parent_dir"
    OCARA="$rel_ocara" "$rel_ocaraunit" "$tests_folder"
    rc=$?
    
    # Revenir au répertoire d'origine
    cd "$original_dir"
    
    if [ $rc -ne 0 ]; then
        echo -e "${RED}FAIL Tests dans $test_dir${RESET}"
        return 1
    fi
    
    echo -e "${GREEN}OK   Tests dans $test_dir${RESET}"
    return 0
}

# ── Exécution avec argument spécifique ────────────────────────────────────────
if [ $# -eq 1 ]; then
    TARGET="$1"
    run_tests "$TARGET"
    exit $?
fi

# ── Exécution par défaut : examples/project/tests ─────────────────────────────
fail=0
failed=""

if ! run_tests "examples/project/tests"; then
    fail=1
    failed="$failed examples/project/tests"
fi

echo ""

# ── Résultat final ────────────────────────────────────────────────────────────
echo "══════════════════════════════════════════════"

if [ $fail -eq 0 ]; then
    echo -e "${GREEN}Tous les tests unitaires ont réussi.${RESET}"
    echo "══════════════════════════════════════════════"
    exit 0
else
    echo -e "${RED}Échecs :${RESET}"
    for f in $failed; do
        echo -e "  ${RED}✗ $f${RESET}"
    done
    echo ""
    echo -e "${RED}Des tests unitaires ont échoué.${RESET}"
    echo "══════════════════════════════════════════════"
    exit 1
fi
