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

# ── Détecte un serveur MySQL/MariaDB joignable (voir builtins/mysql.oc) ───────
# Simple sondage TCP (pas d'authentification) : suffisant pour distinguer
# "aucun serveur disponible ici" (skip propre) de "serveur présent mais requête
# invalide" (vrai échec du test). Voir docs/roadmap.d/qualite-couverture-tests.md
# pour l'exemple de service CI (GitHub Actions) qui rendrait ce test actif.
MYSQL_HOST="${MYSQL_HOST:-127.0.0.1}"
MYSQL_PORT="${MYSQL_PORT:-3306}"
mysql_server_available() {
    (exec 3<>"/dev/tcp/$MYSQL_HOST/$MYSQL_PORT") 2>/dev/null
}

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
    # basename ici : en mode single-target, $name garde le préfixe de sous-
    # répertoire (ex: "builtins/httpserver_static") pour l'affichage, alors que
    # les cas ci-dessous sont nommés sans préfixe (comme en mode suite complète,
    # où $name est déjà un basename) — sans ce basename, ces cas ne matchaient
    # jamais en mode single-target et retombaient sur le run bloquant par défaut.
    case "$(basename "$name")" in
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
        httpserver_static)
            examples/builtins/httpserver_static.sh "$TMP"
            ;;
        advanced_httpserver)
            examples/advanced/httpserver/httpserver.sh "$TMP"
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
    
    # Déterminer le chemin du fichier source
    if [[ "$TARGET" == *.oc ]]; then
        # Si .oc déjà présent, utiliser tel quel
        src="$TARGET"
    elif [[ "$TARGET" == examples/* ]]; then
        # Si commence par examples/, ajouter juste .oc
        src="${TARGET}.oc"
    else
        # Sinon, préfixer avec examples/
        src="examples/${TARGET}.oc"
    fi
    
    if [ ! -f "$src" ]; then
        echo -e "${RED}Erreur : $src n'existe pas${RESET}"
        exit 1
    fi
    
    # Extraire le nom pour l'affichage (enlever examples/ et .oc)
    name="${src#examples/}"
    name="${name%.oc}"
    
    echo "══════════════════════════════════════════════"
    echo " Régression $src"
    echo "══════════════════════════════════════════════"
    
    run_test "$src" "$name"
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

# Test examples/from/import_from.oc — seul point d'entrée exécutable de
# examples/from/ (les autres fichiers de ce dossier ne sont que des classes/
# interfaces importées par celui-ci, pas des programmes autonomes).
echo "══════════════════════════════════════════════"
echo " Régression examples/from/import_from.oc"
echo "══════════════════════════════════════════════"

if ! run_test "examples/from/import_from.oc" "from/import_from"; then
    fail=1
    failed="$failed from/import_from"
fi

echo ""

# Test examples/advanced/httpserver/main.oc — serveur HTTP pur (pas de GUI),
# testé via httpserver.sh (démarrage en fond + requêtes + arrêt), même
# mécanisme que examples/builtins/httpserver.sh. `mini_project` et
# `tauri_httpserver` (les 2 autres projets de examples/advanced/) ouvrent en
# plus une fenêtre Tauri (WebView) et ne sont volontairement pas couverts ici
# — voir docs/roadmap.d/qualite-couverture-tests.md.
echo "══════════════════════════════════════════════"
echo " Régression examples/advanced/httpserver/main.oc"
echo "══════════════════════════════════════════════"

if ! run_test "examples/advanced/httpserver/main.oc" "advanced_httpserver"; then
    fail=1
    failed="$failed advanced_httpserver"
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

    # builtins/mysql.oc a besoin d'un vrai serveur MySQL/MariaDB — absent de
    # cet environnement de dev, on saute proprement plutôt que d'échouer.
    if [ "$name" = "mysql" ] && ! mysql_server_available; then
        echo "SKIP: builtins/mysql (aucun serveur MySQL/MariaDB accessible sur $MYSQL_HOST:$MYSQL_PORT)"
        continue
    fi

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
