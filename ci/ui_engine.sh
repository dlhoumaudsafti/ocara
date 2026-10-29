#!/usr/bin/env bash
# Moteur de rendu desktop d'`ocara.UI` (runtime_tauri/src/ui/engine.js) : joue
# ci/ui_engine_test.js dans Chrome sans écran, sur l'aperçu de
# examples/ui/components.oc.ui. Sans Chrome/Chromium : SKIP.

OCARA="./target/release/ocara"
GREEN='\033[0;32m'
RED='\033[0;31m'
RESET='\033[0m'

CHROME=""
for candidate in google-chrome chromium chromium-browser; do
    if command -v "$candidate" > /dev/null; then
        CHROME="$candidate"
        break
    fi
done
if [ -z "$CHROME" ]; then
    echo "SKIP: ui/engine (ni Chrome ni Chromium)"
    exit 0
fi

dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
if ! $OCARA examples/ui/components.oc.ui --preview -o "$dir/page.html" > /dev/null; then
    echo -e "${RED}FAIL [aperçu] ui/engine${RESET}"
    exit 1
fi
# Le test remplace le démarrage de l'aperçu (même arbre, autre `send`).
sed -i "s#</body>#<script src=\"$PWD/ci/ui_engine_test.js\"></script></body>#" "$dir/page.html"
result=$("$CHROME" --headless=new --disable-gpu --no-sandbox --user-data-dir="$dir/profile" \
    --allow-file-access-from-files --virtual-time-budget=5000 --dump-dom "file://$dir/page.html" 2> /dev/null | grep -o 'UI-ENGINE-[A-Z]*[^<]*')
if [ -z "$result" ] && [ "$(ulimit -v)" != "unlimited" ]; then
    # Chrome réserve des To d'espace d'adressage (V8) : il ne démarre pas
    # sous `ulimit -v`. Lancer alors ./ci/ui_engine.sh seul.
    echo "SKIP: ui/engine (Chrome ne démarre pas sous ulimit -v $(ulimit -v) ; lancer ./ci/ui_engine.sh seul)"
    exit 0
fi
if [ "$result" = "UI-ENGINE-OK" ]; then
    echo -e "${GREEN}PASS: ui/engine${RESET}"
else
    echo -e "${RED}FAIL [moteur] ui/engine : ${result:-aucun résultat}${RESET}"
    exit 1
fi
