# ══════════════════════════════════════════════════════════════════════════════
# Langages comparés à Ocara (source de ci/perf_compare.sh et ci/perf_extra.sh)
# ══════════════════════════════════════════════════════════════════════════════
# Interprétés (`make perf` échoue si Ocara est plus lent) : Python, PHP, Perl.
# Compilés (affichés en référence) : C (gcc -O2), C++ (g++ -O2 -std=c++17),
# Rust (rustc -O, bibliothèque standard seule). Une ligne
# `// perf-libs: -lsqlite3` en tête d'un source C/C++ donne ses bibliothèques.
# Un programme qui sort en erreur pour n = 1 (bibliothèque absente du
# langage, module non installé) est sauté.
# ══════════════════════════════════════════════════════════════════════════════

LANGS=(py php pl c cpp rs)

# Aucun résidu : pas de `__pycache__` (les binaires C, C++ et Rust vont dans
# le dossier temporaire du script, supprimé à la sortie).
export PYTHONDONTWRITEBYTECODE=1
INTERPRETED=(py php pl)

lang_label() {
    case "$1" in
        py) echo "Python" ;; php) echo "PHP" ;; pl) echo "Perl" ;;
        c) echo "C" ;; cpp) echo "C++" ;; rs) echo "Rust" ;;
    esac
}

is_interpreted() {
    [[ " ${INTERPRETED[*]} " == *" $1 "* ]]
}

# lang_cmd <langage> <source sans extension> <dossier de compilation> [build]
# Remplit CMD (commande sans la taille) ; échoue si le langage ne peut pas
# exécuter ce programme. `build` : sans l'exécution d'essai (serveur).
lang_cmd() {
    local lang=$1 base=$2 out=$3 mode=${4:-} src="$2.$1" bin libs
    CMD=()
    [ -f "$src" ] || return 1
    bin="$out/$(basename "$base").$lang.bin"
    libs=$(sed -n 's|^// perf-libs: ||p' "$src")
    case "$lang" in
        py)  command -v python3 > /dev/null && CMD=(python3 "$src") ;;
        php) command -v php > /dev/null && CMD=(php "$src") ;;
        pl)  command -v perl > /dev/null && CMD=(perl "$src") ;;
        c)   command -v gcc > /dev/null && gcc -O2 -w -o "$bin" "$src" -lm $libs 2> /dev/null && CMD=("$bin") ;;
        cpp) command -v g++ > /dev/null && g++ -O2 -std=c++17 -w -o "$bin" "$src" $libs 2> /dev/null && CMD=("$bin") ;;
        rs)  command -v rustc > /dev/null && rustc -O -o "$bin" "$src" 2> /dev/null && CMD=("$bin") ;;
    esac
    [ ${#CMD[@]} -gt 0 ] || return 1
    [ "$mode" = build ] || "${CMD[@]}" 1 > /dev/null 2>&1
}
