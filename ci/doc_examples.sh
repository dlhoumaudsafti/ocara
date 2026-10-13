#!/usr/bin/env bash
# ══════════════════════════════════════════════════════════════════════════════
# Exemples de la documentation (docs/**/*.md)
# ══════════════════════════════════════════════════════════════════════════════
# Vérifie (`ocara --check`) chaque bloc ```ocara COMPLET de la documentation —
# un bloc qui déclare `function main` ; les fragments ne sont pas contrôlés.
# `architecture permissive` est ajouté s'il manque. Un bloc précédé d'une
# ligne `<!-- doc-check: skip -->` (exemple volontairement invalide, ❌) est
# ignoré, ainsi que tout bloc contenant `❌`.
#   ./ci/doc_examples.sh            → tous les fichiers de docs/
#   ./ci/doc_examples.sh docs/X.md  → un seul fichier
# ══════════════════════════════════════════════════════════════════════════════

OCARA="./target/release/ocara"
TMP="${TMPDIR:-/tmp}/oc_doc_examples"
GREEN='\033[0;32m'
RED='\033[0;31m'
RESET='\033[0m'

if [ ! -x "$OCARA" ]; then
    echo -e "${RED}Erreur : $OCARA introuvable — exécutez d'abord : make build${RESET}"
    exit 1
fi

rm -rf "$TMP"
mkdir -p "$TMP"

files=("$@")
if [ ${#files[@]} -eq 0 ]; then
    mapfile -t files < <(find docs -name "*.md" -not -path "docs/roadmap.d/*" | sort)
fi

pass=0
fail=0
for md in "${files[@]}"; do
    # Extrait chaque bloc ```ocara dans un fichier numéroté.
    awk -v dir="$TMP" -v base="$(echo "$md" | tr '/.' '__')" '
        /^<!-- doc-check: skip -->/ { skip = 1; next }
        /^```ocara[[:space:]]*$/ { inblock = 1; n++; out = sprintf("%s/%s_%03d.oc", dir, base, n); line = NR + 1; if (skip) out = ""; skip = 0; if (out != "") print "// " FILENAME ":" line > out; next }
        /^```/ && inblock { inblock = 0; if (out != "") close(out); next }
        inblock && out != "" { print > out }
        !inblock && !/^[[:space:]]*$/ { skip = 0 }
    ' "$md"
done

for oc in "$TMP"/*.oc; do
    [ -e "$oc" ] || continue
    grep -q "function main" "$oc" || continue
    grep -q "❌" "$oc" && continue
    origin=$(head -1 "$oc" | sed 's|^// ||')
    if ! grep -q "^architecture " "$oc"; then
        # Après un éventuel `namespace` (toujours en première position).
        if grep -q "^namespace " "$oc"; then
            sed -i '0,/^namespace /{/^namespace /a architecture permissive
}' "$oc"
        else
            sed -i '1a architecture permissive' "$oc"
        fi
    fi
    if out=$("$OCARA" "$oc" --check 2>&1); then
        pass=$((pass + 1))
    else
        fail=$((fail + 1))
        echo -e "${RED}FAIL${RESET} $origin"
        echo "$out" | grep "error" | head -3 | sed 's/^/    /'
    fi
done

echo
echo -e "Exemples de documentation : ${GREEN}${pass} OK${RESET}, ${RED}${fail} en échec${RESET}"
[ "$fail" -eq 0 ]
