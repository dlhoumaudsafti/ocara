#!/usr/bin/env bash
# Programmes `ocara.UI` (examples/ui/*.oc) : compilés et liés ; l'auto-test
# (selftest.oc, qui ouvre une fenêtre et se ferme seul) est joué quand un
# écran est disponible, sa sortie comparée à selftest.expected.

OCARA="./target/release/ocara"
GREEN='\033[0;32m'
RED='\033[0;31m'
RESET='\033[0m'
status=0
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT

for src in examples/ui/*.oc; do
    name="ui/$(basename "$src" .oc)"
    if (cd examples/ui && "../../$OCARA" "$(basename "$src")" -o "$dir/$(basename "$src" .oc)" > /dev/null); then
        echo -e "${GREEN}PASS [compile]: $name${RESET}"
    else
        echo -e "${RED}FAIL [compile] $name${RESET}"
        status=1
    fi
done

if [ -z "$DISPLAY" ] && [ -z "$WAYLAND_DISPLAY" ]; then
    echo "SKIP: ui/selftest (aucun écran)"
    exit $status
fi
output=$(cd examples/ui && timeout 60 "$dir/selftest" 2> /dev/null)
if [ "$output" = "$(cat examples/ui/selftest.expected)" ]; then
    echo -e "${GREEN}PASS: ui/selftest${RESET}"
elif [ -z "$output" ] && [ "$(ulimit -v)" != "unlimited" ]; then
    # WebKitGTK réserve de très grands espaces d'adressage : il ne démarre
    # pas sous `ulimit -v`. Lancer alors ./ci/ui_app.sh seul.
    echo "SKIP: ui/selftest (WebKit ne démarre pas sous ulimit -v $(ulimit -v) ; lancer ./ci/ui_app.sh seul)"
else
    echo -e "${RED}FAIL [sortie] ui/selftest${RESET}"
    diff <(echo "$output") examples/ui/selftest.expected
    status=1
fi
exit $status
