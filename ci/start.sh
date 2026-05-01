#!/usr/bin/env bash
# set -e

# ══════════════════════════════════════════════════════════════════════════════
# CI Ocara - Pipeline d'intégration continue
# ══════════════════════════════════════════════════════════════════════════════
# Ce script exécute le pipeline CI pour chaque fichier individuellement :
#   Pour chaque exemple :
#     1. Linter (ocaracs) - style et qualité du code
#     2. Vérification sémantique (--check) - types et symboles
#     3. Dump IR (--dump) - représentation intermédiaire
#     4. Tests unitaires + couverture (ocaraunit --coverage) - validation complète
# ══════════════════════════════════════════════════════════════════════════════

OCARA="./target/release/ocara"
OCARAUNIT="./target/release/ocaraunit"
OCARACS="./target/release/ocaracs"
DUMP_DIR="./ci/dumps"
REPORT_FILE="./ci/ci-report.txt"

# Couleurs
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

# Compteurs globaux
total_files=0
passed_files=0
failed_files=0
failed_file_list=()

# ── Vérifier les binaires ────────────────────────────────────────────────────
if [ ! -x "$OCARA" ]; then
    echo -e "${RED}✗ Erreur : $OCARA n'existe pas ou n'est pas exécutable${RESET}"
    echo "Exécutez : make build"
    exit 1
fi

if [ ! -x "$OCARAUNIT" ]; then
    echo -e "${RED}✗ Erreur : $OCARAUNIT n'existe pas ou n'est pas exécutable${RESET}"
    echo "Exécutez : make build-tools"
    exit 1
fi

if [ ! -x "$OCARACS" ]; then
    echo -e "${RED}✗ Erreur : $OCARACS n'existe pas ou n'est pas exécutable${RESET}"
    echo "Exécutez : make build-tools"
    exit 1
fi

# Créer le dossier de dumps
mkdir -p "$DUMP_DIR"

# Initialiser le rapport
echo "══════════════════════════════════════════════════════════════════════════════" > "$REPORT_FILE"
echo "  OCARA CI - RAPPORT D'INTÉGRATION CONTINUE" >> "$REPORT_FILE"
echo "  Date: $(date '+%Y-%m-%d %H:%M:%S')" >> "$REPORT_FILE"
echo "══════════════════════════════════════════════════════════════════════════════" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

# ══════════════════════════════════════════════════════════════════════════════
# Fonction : Traiter un fichier (4 étapes séquentielles)
# ══════════════════════════════════════════════════════════════════════════════
process_file() {
    local src="$1"
    local name="$2"
    local file_status=0
    
    total_files=$((total_files + 1))
    
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
    echo -e "${BOLD}  Fichier : ${name}${RESET}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
    
    echo "" >> "$REPORT_FILE"
    echo "────────────────────────────────────────────────────────────────────────────────" >> "$REPORT_FILE"
    echo "Fichier : $name" >> "$REPORT_FILE"
    echo "────────────────────────────────────────────────────────────────────────────────" >> "$REPORT_FILE"
    
    # ── Étape 1 : Linter (ocaracs) ──────────────────────────────────────────
    echo -e "  ${BOLD}[1/4]${RESET} Linter..."
    local lint_output=$(mktemp)
    if $OCARACS "$src" > "$lint_output" 2>&1; then
        echo -e "        ${GREEN}✓ Style OK${RESET}"
        echo "  [1/4] Linter : ✓ OK" >> "$REPORT_FILE"
    else
        echo -e "        ${YELLOW}⚠ Warnings de style${RESET}"
        echo "  [1/4] Linter : ✗ ERREUR (warnings)" >> "$REPORT_FILE"
        echo "" >> "$REPORT_FILE"
        echo "  Sortie de ocaracs :" >> "$REPORT_FILE"
        sed 's/^/    /' "$lint_output" >> "$REPORT_FILE"
        file_status=1
    fi
    rm -f "$lint_output"
    
    # ── Étape 2 : Vérification sémantique (--check) ─────────────────────────
    echo -e "  ${BOLD}[2/4]${RESET} Vérification sémantique..."
    local check_output=$(mktemp)
    
    # Cas spécial : 21_errors doit échouer
    if [[ "$name" == *"21_errors"* ]]; then
        $OCARA "$src" --check > "$check_output" 2>&1
        rc=$?
        if [ $rc -eq 0 ]; then
            echo -e "        ${RED}✗ ERREUR : devait échouer${RESET}"
            echo "  [2/4] Check : ✗ ERREUR (devait échouer)" >> "$REPORT_FILE"
            file_status=1
        else
            echo -e "        ${GREEN}✓ Échec attendu${RESET}"
            echo "  [2/4] Check : ✓ Échec attendu" >> "$REPORT_FILE"
            echo "" >> "$REPORT_FILE"
            echo "  Sortie de ocara --check :" >> "$REPORT_FILE"
            sed 's/^/    /' "$check_output" >> "$REPORT_FILE"
        fi
    else
        if $OCARA "$src" --check > "$check_output" 2>&1; then
            echo -e "        ${GREEN}✓ Sémantique OK${RESET}"
            echo "  [2/4] Check : ✓ OK" >> "$REPORT_FILE"
        else
            echo -e "        ${RED}✗ Erreurs sémantiques${RESET}"
            echo "  [2/4] Check : ✗ ERREUR" >> "$REPORT_FILE"
            echo "" >> "$REPORT_FILE"
            echo "  Sortie de ocara --check :" >> "$REPORT_FILE"
            sed 's/^/    /' "$check_output" >> "$REPORT_FILE"
            file_status=1
        fi
    fi
    rm -f "$check_output"
    
    # ── Étape 3 : Dump IR (--dump) ──────────────────────────────────────────
    echo -e "  ${BOLD}[3/4]${RESET} Dump IR..."
    local dump_file="$DUMP_DIR/${name//\//_}.dump"
    local dump_errors=$(mktemp)
    
    if [[ "$name" == *"21_errors"* ]]; then
        echo -e "        ${YELLOW}⊘ Skip (erreurs intentionnelles)${RESET}"
        echo "  [3/4] Dump  : ⊘ Skip" >> "$REPORT_FILE"
    else
        if $OCARA "$src" --dump > "$dump_file" 2>"$dump_errors"; then
            echo -e "        ${GREEN}✓ Dump sauvegardé → ${dump_file#./}${RESET}"
            echo "  [3/4] Dump  : ✓ OK → ${dump_file#./}" >> "$REPORT_FILE"
        else
            echo -e "        ${RED}✗ Échec du dump${RESET}"
            echo "  [3/4] Dump  : ✗ ERREUR" >> "$REPORT_FILE"
            if [ -s "$dump_errors" ]; then
                echo "" >> "$REPORT_FILE"
                echo "  Sortie de ocara --dump :" >> "$REPORT_FILE"
                sed 's/^/    /' "$dump_errors" >> "$REPORT_FILE"
            fi
            file_status=1
        fi
    fi
    rm -f "$dump_errors"
    
    # ── Étape 4 : Tests unitaires + Couverture (ocaraunit --coverage) ───────
    echo -e "  ${BOLD}[4/4]${RESET} Tests unitaires + couverture..."
    
    # Construire le nom du fichier de test correspondant
    local test_file=""
    local base_name=$(basename "$name")
    
    if [[ "$src" == examples/project/* ]]; then
        # Pour project/main.oc → mainTest.oc dans examples/project/tests/
        test_file="examples/project/tests/${base_name}Test.oc"
    elif [[ "$src" == examples/builtins/* ]]; then
        # Pas de tests unitaires pour les exemples builtins → ÉCHEC
        echo -e "        ${RED}✗ Pas de tests unitaires pour les builtins${RESET}"
        echo "  [4/4] Tests : ✗ ERREUR (builtins sans tests)" >> "$REPORT_FILE"
        file_status=1
    elif [[ "$src" == examples/* ]]; then
        # Pour examples/01_variables.oc → 01_variablesTest.oc dans examples/tests/
        test_file="examples/tests/${base_name}Test.oc"
    fi
    
    # Vérifier si le fichier de test existe
    if [ -n "$test_file" ]; then
        if [ -f "$test_file" ]; then
            local coverage_output=$(mktemp)
            echo "        → Test file: ${test_file#./}"
            echo "  Test associé : ${test_file#./}" >> "$REPORT_FILE"
            
            if $OCARAUNIT --coverage "$src" "$test_file" > "$coverage_output" 2>&1; then
                # Extraire le pourcentage de couverture globale
                local coverage_percent=$(grep "Couverture globale :" "$coverage_output" | grep -oE "[0-9]+\.[0-9]+%" | head -n 1 | tr -d '%')
                
                # Vérifier si la couverture est à 100%
                if [ -n "$coverage_percent" ] && [ "$(echo "$coverage_percent == 100.0" | bc -l)" -eq 1 ]; then
                    echo -e "        ${GREEN}✓ Tests passés - Couverture: ${coverage_percent}%${RESET}"
                    echo "  [4/4] Tests : ✓ OK (Couverture: ${coverage_percent}%)" >> "$REPORT_FILE"
                    # Ne pas afficher la sortie si couverture à 100%
                else
                    echo -e "        ${RED}✗ Couverture incomplète: ${coverage_percent}%${RESET}"
                    echo "  [4/4] Tests : ✗ ERREUR (Couverture incomplète: ${coverage_percent}%)" >> "$REPORT_FILE"
                    echo "" >> "$REPORT_FILE"
                    echo "  Sortie de ocaraunit --coverage :" >> "$REPORT_FILE"
                    sed 's/^/    /' "$coverage_output" >> "$REPORT_FILE"
                    file_status=1
                fi
            else
                echo -e "        ${RED}✗ Tests échoués${RESET}"
                echo "  [4/4] Tests : ✗ ERREUR" >> "$REPORT_FILE"
                echo "" >> "$REPORT_FILE"
                echo "  Sortie de ocaraunit --coverage :" >> "$REPORT_FILE"
                sed 's/^/    /' "$coverage_output" >> "$REPORT_FILE"
                file_status=1
            fi
            rm -f "$coverage_output"
        else
            echo -e "        ${RED}✗ Fichier de test non trouvé : ${test_file#./}${RESET}"
            echo "  [4/4] Tests : ✗ ERREUR (fichier de test manquant : ${test_file#./})" >> "$REPORT_FILE"
            file_status=1
        fi
    fi
    
    # ── Résultat final du fichier ───────────────────────────────────────────
    if [ $file_status -eq 0 ]; then
        echo -e "  ${BOLD}${GREEN}✓ SUCCÈS${RESET}"
        echo "  Résultat : ✓ SUCCÈS" >> "$REPORT_FILE"
        passed_files=$((passed_files + 1))
    else
        echo -e "  ${BOLD}${RED}✗ ÉCHEC${RESET}"
        echo "  Résultat : ✗ ÉCHEC" >> "$REPORT_FILE"
        failed_files=$((failed_files + 1))
        failed_file_list+=("$name")
    fi
    
    return $file_status
}

# ══════════════════════════════════════════════════════════════════════════════
# Traiter tous les fichiers
# ══════════════════════════════════════════════════════════════════════════════

OCARAUNIT --clear > /dev/null 2>&1

# Exemples NN_*.oc
for src in examples/[0-9][0-9]_*.oc; do
    [ -f "$src" ] || continue
    name=$(basename "$src" .oc)
    process_file "$src" "$name"
done

# # Exemple project/main.oc
# if [ -f "examples/project/main.oc" ]; then
#     process_file "examples/project/main.oc" "project/main"
# fi

# # Exemples builtins/*.oc
# for src in examples/builtins/*.oc; do
#     [ -f "$src" ] || continue
#     name="builtins/$(basename "$src" .oc)"
#     process_file "$src" "$name"
# done

# ══════════════════════════════════════════════════════════════════════════════
# RAPPORT FINAL
# ══════════════════════════════════════════════════════════════════════════════
echo ""
echo -e "${BOLD}${BLUE}╔════════════════════════════════════════════════════════════════════════════╗${RESET}"
echo -e "${BOLD}${BLUE}║ RAPPORT FINAL                                                              ║${RESET}"
echo -e "${BOLD}${BLUE}╚════════════════════════════════════════════════════════════════════════════╝${RESET}"
echo ""
echo "══════════════════════════════════════════════════════════════════════════════" >> "$REPORT_FILE"
echo "  RAPPORT FINAL" >> "$REPORT_FILE"
echo "══════════════════════════════════════════════════════════════════════════════" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

echo "Fichiers traités : $total_files" >> "$REPORT_FILE"
echo "Fichiers réussis : $passed_files" >> "$REPORT_FILE"
echo "Fichiers échoués : $failed_files" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

echo -e "  ${BOLD}Fichiers traités :${RESET} $total_files"
echo -e "  ${BOLD}Fichiers réussis :${RESET} ${GREEN}$passed_files${RESET}"
echo -e "  ${BOLD}Fichiers échoués :${RESET} ${RED}$failed_files${RESET}"
echo ""

if [ $failed_files -eq 0 ]; then
    echo -e "${GREEN}${BOLD}✓ Pipeline CI : SUCCÈS${RESET}"
    echo "" >> "$REPORT_FILE"
    echo "✓ Pipeline CI : SUCCÈS" >> "$REPORT_FILE"
    echo "" >> "$REPORT_FILE"
    echo "Rapport sauvegardé : $REPORT_FILE"
    echo "Dumps sauvegardés  : $DUMP_DIR"
    exit 0
else
    echo -e "${RED}${BOLD}✗ Pipeline CI : ÉCHEC ($failed_files fichier(s) échoué(s))${RESET}"
    echo "" >> "$REPORT_FILE"
    echo "✗ Pipeline CI : ÉCHEC ($failed_files fichier(s) échoué(s))" >> "$REPORT_FILE"
    echo "" >> "$REPORT_FILE"
    
    # Liste des fichiers en échec
    if [ ${#failed_file_list[@]} -gt 0 ]; then
        echo "Fichiers en échec :" >> "$REPORT_FILE"
        for file in "${failed_file_list[@]}"; do
            echo "  - $file" >> "$REPORT_FILE"
        done
        echo "" >> "$REPORT_FILE"
        
        echo ""
        echo -e "${BOLD}${YELLOW}Fichiers en échec :${RESET}"
        for file in "${failed_file_list[@]}"; do
            echo -e "  ${RED}•${RESET} $file"
        done
        echo ""
    fi
    
    echo "Rapport sauvegardé : $REPORT_FILE"
    echo "Dumps sauvegardés  : $DUMP_DIR"
    exit 1
fi