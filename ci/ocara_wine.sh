#!/bin/bash
# Compilateur de substitution pour ocaraunit (variable OCARA) : compile sous
# Wine avec ocara.exe, lie avec MinGW-w64 comme docs/windows.md §4, et laisse
# à la place du binaire un script qui l'exécute sous Wine.
# Utilisé par `make tests-windows` (après `make build-windows`).
#
# Appel par ocaraunit : ocara_wine.sh <source.oc> -o <binaire> [options…]

ROOT=$(cd "$(dirname "$0")/.." && pwd)
WIN="$ROOT/target/x86_64-pc-windows-gnu/release"
CC=${WINDOWS_CC:-x86_64-w64-mingw32-gcc}

src=$1
shift
args=()
bin=
while [ $# -gt 0 ]; do
    if [ "$1" = "-o" ]; then bin=$2; shift 2; else args+=("$1"); shift; fi
done

WINEDEBUG=-all wine "$WIN/ocara.exe" "$src" --no-link -o "$bin" "${args[@]}" || exit 1
"$CC" "$bin.o" "$WIN/libocara_runtime.a" "$WIN/libocara_runtime_image.a" -o "$bin.exe" -lm \
    -Wl,--allow-multiple-definition -Wl,--gc-sections -Wl,--as-needed \
    -lws2_32 -lbcrypt -luserenv -lntdll || exit 1
rm -f "$bin.o"
printf '#!/bin/bash\nWINEDEBUG=-all exec wine "%s" "$@"\n' "$bin.exe" > "$bin"
chmod +x "$bin"
