OCARA   := ./target/release/ocara
TMP     := /tmp/oc_regression
GREEN   := \033[0;32m
RED     := \033[0;31m
RESET   := \033[0m

# Argument optionnel : make regression builtins/io ; make android-simulator <apk>
_TARGET := $(filter-out build build-dev build-tools build-tools-dev build-all build-all-dev pkgconfig-shim test tests regression lint-examples tests-examples clean clean-tools clean-all help install install-tools install-all uninstall uninstall-tools uninstall-all build-runtime-android build-runtime-sdl-android build-jni-bridge-android android-simulator build-runtime-windows build-windows,$(MAKECMDGOALS))

.PHONY: build build-dev build-tools build-tools-dev build-all build-all-dev pkgconfig-shim test tests regression ci lint-examples tests-examples clean clean-tools clean-all help install install-tools install-all uninstall uninstall-tools uninstall-all build-runtime-android build-runtime-sdl-android build-jni-bridge-android android-simulator build-runtime-windows build-windows $(_TARGET)

# ── Aide ──────────────────────────────────────────────────────────────────────
help:
	@echo "Usage : make <cible> [argument]"
	@echo ""
	@echo "  build                   Compile le runtime + le compilateur Ocara (release, strict)"
	@echo "  build-dev               Compile le compilateur en mode debug (strict)"
	@echo "  build-tools             Compile les outils (ocaracs, ocaraunit) (release, strict)"
	@echo "  build-tools-dev         Compile les outils en mode debug (strict)"
	@echo "  build-all               Compile tout : ocara + outils (release)"
	@echo "  build-all-dev           Compile tout en mode debug"
	@echo "  test                    Lance les tests unitaires Cargo (cargo test)"
	@echo "  regression              Lance la régression complète (tous les exemples)"
	@echo "  regression <chemin>     Lance uniquement examples/<chemin>.oc"
	@echo "                            ex: make regression builtins/io"
	@echo "                            ex: make regression 07_loops"
	@echo "                            ex: make regression project/main"
	@echo "  ci                      Lance le pipeline CI complet (check + dump + tests + lint)"
	@echo "  lint-examples           Lance ocaracs sur tous les exemples"
	@echo "  tests-examples          Lance ocaraunit sur les exemples"
	@echo "  install                 Installe ocara dans /usr/local/bin/"
	@echo "  install-tools           Installe les outils dans /usr/local/bin/"
	@echo "  install-all             Installe tout (install + install-tools)"
	@echo "  uninstall               Supprime ocara de /usr/local/bin/"
	@echo "  uninstall-tools         Supprime les outils de /usr/local/bin/"
	@echo "  uninstall-all           Désinstalle tout (uninstall + uninstall-tools)"
	@echo "  build-runtime-android   Cross-compile ocara_runtime pour aarch64-linux-android"
	@echo "                            (nécessite ANDROID_NDK_HOME, voir docs/roadmap.d/packaging-android.md)"
	@echo "  build-runtime-sdl-android  Cross-compile ocara_runtime_sdl (SDL3) pour aarch64-linux-android"
	@echo "                            (nécessite ANDROID_NDK_HOME + cmake, sous-chantier 4)"
	@echo "  build-jni-bridge-android   Cross-compile le pont JNI (runtime_android_jni)"
	@echo "                            (nécessite ANDROID_NDK_HOME, voir packaging/android/README.md)"
	@echo "  android-simulator <apk> Installe et lance un APK sur un émulateur Android"
	@echo "                            (démarre l'émulateur si besoin, fenêtre visible ;"
	@echo "                            désinstalle/réinstalle si déjà présent — nécessite"
	@echo "                            ANDROID_HOME). ANDROID_SERIAL=<serial> pour cibler un"
	@echo "                            appareil déjà connecté (adb devices) au lieu de l'émulateur."
	@echo "  build-runtime-windows   Cross-compile ocara_runtime pour x86_64-pc-windows-gnu"
	@echo "                            (nécessite gcc-mingw-w64-x86-64, voir docs/roadmap.d/packaging-windows.md)"
	@echo "  build-windows           Cross-compile ocara lui-même (ocara.exe) pour x86_64-pc-windows-gnu"
	@echo "  clean                   Supprime les artefacts de compilation d'ocara"
	@echo "  clean-tools             Supprime les artefacts de compilation des outils"
	@echo "  clean-all               Supprime tous les artefacts (clean + clean-tools)"
	@echo "  help                    Affiche ce message"

# Absorber l'argument positionnel pour éviter "No rule to make target"
ifneq ($(_TARGET),)
$(_TARGET):
	@:
endif

# ── Shim pkg-config pour le builtin Tauri ────────────────────────────────────
# Sur Ubuntu ≥ 24.04, libwebkit2gtk-4.0-dev / libjavascriptcoregtk-4.0-dev n'existent
# plus dans les dépôts (remplacés par la 4.1, ABI compatible pour wry 0.24 / tauri 1.x).
# On génère ici des .pc locaux (jamais dans le système) plutôt que de symlinker /usr/lib,
# et on les priorise via PKG_CONFIG_PATH au moment du build. Rien à committer :
# régénéré à chaque `make build` si besoin, ignoré par git (.pkgconfig-shim/).
PKGCONFIG_SHIM := $(CURDIR)/.pkgconfig-shim

# Phony (pas un vrai target-fichier) : réévalué à chaque build, coût négligeable
# (juste des requêtes pkg-config + symlinks), pour rattraper une install apt faite entre-temps.
pkgconfig-shim:
	@mkdir -p $(PKGCONFIG_SHIM)
	@for pkg in webkit2gtk javascriptcoregtk; do \
	    if ! pkg-config --exists $$pkg-4.0 2>/dev/null && pkg-config --exists $$pkg-4.1 2>/dev/null; then \
	        SRC="$$(pkg-config --variable=pcfiledir $$pkg-4.1)/$$pkg-4.1.pc"; \
	        ln -sf "$$SRC" "$(PKGCONFIG_SHIM)/$$pkg-4.0.pc"; \
	        echo "  shim pkg-config: $$pkg-4.0.pc -> $$SRC"; \
	    fi; \
	done

# ── Compilation du compilateur + runtime(s) ──────────────────────────────────
# Les runtimes doivent être compilés en premier : build.rs les embarque dans le
# binaire. ocara_runtime_tauri est un crate SÉPARÉ de ocara_runtime (voir
# src/codegen/link.rs) : lié seulement pour les programmes qui importent
# ocara.Tauri, il a donc aussi besoin du shim pkg-config GTK/WebKit à la
# compilation (il dépend directement du crate `tauri`).
# La contrainte -j1 historiquement documentée ici (Cranelift lourd à
# compiler en parallèle, SIGKILL OOM) ne s'applique plus/pas sur cette
# machine : -j4 a tourné sans incident sur des dizaines de builds successifs
# (voir docs/roadmap.d/packaging-build-cargo.md) — repasser à -j1 si l'OOM
# est un jour de nouveau observé sur une machine à mémoire plus limitée.
build: pkgconfig-shim
	PKG_CONFIG_PATH="$(PKGCONFIG_SHIM):$$PKG_CONFIG_PATH" RUSTFLAGS="-D warnings" cargo build --release -p ocara_runtime -j4
	PKG_CONFIG_PATH="$(PKGCONFIG_SHIM):$$PKG_CONFIG_PATH" RUSTFLAGS="-D warnings" cargo build --release -p ocara_runtime_tauri -j4
	# ocara_runtime_sdl : pas de pkg-config (SDL3 compilé depuis les sources et
	# lié statiquement, voir runtime_sdl/Cargo.toml) — nécessite cmake + un
	# compilateur C (+ headers X11 dev sur Linux), voir README.
	RUSTFLAGS="-D warnings" cargo build --release -p ocara_runtime_sdl -j4
	RUSTFLAGS="-D warnings" cargo build --release -p ocara -j4

build-dev: pkgconfig-shim
	PKG_CONFIG_PATH="$(PKGCONFIG_SHIM):$$PKG_CONFIG_PATH" RUSTFLAGS="-D warnings" cargo build -p ocara_runtime -j4
	PKG_CONFIG_PATH="$(PKGCONFIG_SHIM):$$PKG_CONFIG_PATH" RUSTFLAGS="-D warnings" cargo build -p ocara_runtime_tauri -j4
	RUSTFLAGS="-D warnings" cargo build -p ocara_runtime_sdl -j4
	RUSTFLAGS="-D warnings" cargo build -p ocara -j4

# ── Cross-compilation Android (sous-chantier 3, packaging-android.md) ───────
# `ocara_runtime` seul (pas runtime_tauri : GTK n'a pas d'équivalent Android,
# hors périmètre ; pas runtime_sdl : sous-chantier 4, backend Android non
# vérifié pour sdl3-sys). Le clang du NDK sert de compilateur C croisé pour
# les dépendances C vendorisées (OpenSSL "vendored", SQLite "bundled", zlib
# "static") — sans lui leurs build.rs invoqueraient le `cc` de l'hôte, qui
# produit du x86_64, pas de l'AArch64 Bionic. Niveau d'API 24 (Android 7.0,
# choisi comme plancher raisonnable — aucune contrainte connue plus stricte
# ici, ajustable si besoin).
#
# ANDROID_TARGET : triple Rust/NDK visé, overridable (`make build-runtime-android
# ANDROID_TARGET=x86_64-linux-android`) — ex. pour tester dans un émulateur
# accéléré KVM (voir `android-simulator` plus bas), qui a besoin d'un `.so`
# x86_64, pas seulement arm64-v8a. Fonctionne tel quel pour `aarch64-linux-android`
# et `x86_64-linux-android` : sur ces deux triples précis, le préfixe clang du
# NDK est identique au triple Rust lui-même (ce n'est PAS vrai pour
# `armv7-linux-androideabi`, qui utilise le préfixe `armv7a-linux-androideabi`
# — non géré ici, seuls aarch64/x86_64 sont couverts par ces cibles Make).
ANDROID_TARGET     ?= aarch64-linux-android
ANDROID_TARGET_ENV := $(subst -,_,$(ANDROID_TARGET))
ANDROID_API   := 24
ANDROID_NDK_CLANG_DIR := $(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/linux-x86_64/bin
ANDROID_CC      := $(ANDROID_NDK_CLANG_DIR)/$(ANDROID_TARGET)$(ANDROID_API)-clang
ANDROID_AR      := $(ANDROID_NDK_CLANG_DIR)/llvm-ar
# NDK ≥ r23 (tout-LLVM) ne fournit plus les wrapper binutils préfixés par le
# triple (`aarch64-linux-android-ranlib`) — seuls les outils `llvm-*` existent.
# La crate `cc` (utilisée par openssl-src/libz-sys) essaie `llvm-ranlib` SANS
# chemin complet (résolu via PATH) avant de retomber sur le nom préfixé
# inexistant — d'où l'échec observé ("aarch64-linux-android-ranlib: not
# found") si `llvm-ranlib` n'est pas déjà sur le PATH. Fixé explicitement ici
# plutôt que d'exiger que l'appelant modifie son PATH.
ANDROID_RANLIB  := $(ANDROID_NDK_CLANG_DIR)/llvm-ranlib

build-runtime-android:
	@if [ -z "$(ANDROID_NDK_HOME)" ]; then \
	    echo "ANDROID_NDK_HOME non défini (racine du NDK Android requise)"; exit 1; \
	fi
	CC_$(ANDROID_TARGET_ENV)="$(ANDROID_CC)" \
	AR_$(ANDROID_TARGET_ENV)="$(ANDROID_AR)" \
	RANLIB_$(ANDROID_TARGET_ENV)="$(ANDROID_RANLIB)" \
	CARGO_TARGET_$(shell echo $(ANDROID_TARGET_ENV) | tr a-z A-Z)_LINKER="$(ANDROID_CC)" \
	RUSTFLAGS="-D warnings" cargo build --release -p ocara_runtime --target $(ANDROID_TARGET) -j4

# `ocara_runtime_android_jni` (packaging-android-webview-hybrid.md) : pont JNI
# générique, indépendant de tout import Ocara — voir packaging/android/README.md
# pour l'usage complet (liaison d'un .so chargeable par une Activity Android).
build-jni-bridge-android:
	@if [ -z "$(ANDROID_NDK_HOME)" ]; then \
	    echo "ANDROID_NDK_HOME non défini (racine du NDK Android requise)"; exit 1; \
	fi
	CC_$(ANDROID_TARGET_ENV)="$(ANDROID_CC)" \
	AR_$(ANDROID_TARGET_ENV)="$(ANDROID_AR)" \
	RANLIB_$(ANDROID_TARGET_ENV)="$(ANDROID_RANLIB)" \
	CARGO_TARGET_$(shell echo $(ANDROID_TARGET_ENV) | tr a-z A-Z)_LINKER="$(ANDROID_CC)" \
	RUSTFLAGS="-D warnings" cargo build --release -p ocara_runtime_android_jni --target $(ANDROID_TARGET) -j4

# `ocara_runtime_sdl` (sous-chantier 4, packaging-android.md) : SDL3 + image/
# ttf/mixer compilés depuis les sources via cmake (feature
# "build-from-source-static", voir runtime_sdl/Cargo.toml) — `sdl3-sys` gère
# déjà lui-même les défines cmake spécifiques Android (ANDROID_ABI,
# CMAKE_SYSTEM_NAME, voir sdl3-sys/build-common.rs) mais ne fournit PAS
# `CMAKE_TOOLCHAIN_FILE` : sans lui, cmake ignore silencieusement les defines
# Android et essaie de compiler pour l'hôte. Fourni ici explicitement (la
# crate `cmake` le lit comme variable d'environnement quand aucun
# `.define(...)` explicite n'existe déjà, voir cmake-rs/src/lib.rs).
# CXX_ : au moins un codec de SDL_mixer (ex: "gme", chiptune) est en C++ ; sans
# CXX explicite, `cc`-rs essaie de deviner `aarch64-linux-android-clang++`
# (convention binutils absente des NDK ≥ r23, même piège que RANLIB plus haut).
build-runtime-sdl-android:
	@if [ -z "$(ANDROID_NDK_HOME)" ]; then \
	    echo "ANDROID_NDK_HOME non défini (racine du NDK Android requise)"; exit 1; \
	fi
	CC_$(ANDROID_TARGET_ENV)="$(ANDROID_CC)" \
	CXX_$(ANDROID_TARGET_ENV)="$(ANDROID_CC)++" \
	AR_$(ANDROID_TARGET_ENV)="$(ANDROID_AR)" \
	RANLIB_$(ANDROID_TARGET_ENV)="$(ANDROID_RANLIB)" \
	CARGO_TARGET_$(shell echo $(ANDROID_TARGET_ENV) | tr a-z A-Z)_LINKER="$(ANDROID_CC)" \
	CMAKE_TOOLCHAIN_FILE="$(ANDROID_NDK_HOME)/build/cmake/android.toolchain.cmake" \
	RUSTFLAGS="-D warnings" cargo build --release -p ocara_runtime_sdl --target $(ANDROID_TARGET) -j1

# ── Cross-compilation Windows (packaging-windows.md) ─────────────────────────
# `x86_64-w64-mingw32-gcc` (paquet `gcc-mingw-w64-x86-64`) sert de compilateur
# C croisé pour les dépendances C vendorisées d'ocara_runtime (OpenSSL
# "vendored", SQLite "bundled", zlib "static") — sans lui leurs build.rs
# invoqueraient le `cc` de l'hôte, qui produit du x86_64 Linux (ELF), pas du
# Windows (PE/COFF). 64 bits UNIQUEMENT (`x86_64-pc-windows-gnu`) — 32 bits
# (`i686-*`) explicitement hors périmètre, jamais visé par ce projet.
# `-j4` : même choix que `build` ci-dessus (voir son commentaire) — jamais
# encore vu d'OOM sur cette cible non plus, repasser à `-j1` si observé.
WINDOWS_TARGET := x86_64-pc-windows-gnu
WINDOWS_CC     := x86_64-w64-mingw32-gcc
WINDOWS_AR     := x86_64-w64-mingw32-ar

build-runtime-windows:
	@if ! command -v $(WINDOWS_CC) >/dev/null 2>&1; then \
	    echo "$(WINDOWS_CC) introuvable — installer le paquet gcc-mingw-w64-x86-64 (voir docs/roadmap.d/packaging-windows.md)"; exit 1; \
	fi
	CC_x86_64_pc_windows_gnu="$(WINDOWS_CC)" \
	AR_x86_64_pc_windows_gnu="$(WINDOWS_AR)" \
	CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$(WINDOWS_CC)" \
	RUSTFLAGS="-D warnings" cargo build --release -p ocara_runtime --target $(WINDOWS_TARGET) -j4

# `ocara` lui-même cross-compilé pour Windows (contrairement à Android, où
# seuls des PROGRAMMES Ocara étaient cross-compilés via `--target` sur le
# `ocara` de l'hôte — jamais le compilateur lui-même) : son propre build.rs
# embarque `libocara_runtime.a` (voir target_release_dir(), build.rs racine) —
# `build-runtime-windows` doit donc avoir tourné en premier, sans quoi
# build.rs le déclenche lui-même automatiquement (mêmes limites que `cargo
# build -p ocara` seul côté hôte, voir docs/roadmap.d/packaging-build-cargo.md).
# `ocara_runtime_tauri`/`ocara_runtime_sdl` (GTK/SDL3) ne sont PAS encore
# vérifiés pour cette cible — non couverts ici, voir packaging-windows.md.
build-windows: build-runtime-windows
	CC_x86_64_pc_windows_gnu="$(WINDOWS_CC)" \
	AR_x86_64_pc_windows_gnu="$(WINDOWS_AR)" \
	CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$(WINDOWS_CC)" \
	RUSTFLAGS="-D warnings" cargo build --release -p ocara --target $(WINDOWS_TARGET) -j4
	@echo "ocara.exe : target/$(WINDOWS_TARGET)/release/ocara.exe"

# ── Émulateur Android : installer + lancer un APK pour le tester ────────────
# `make android-simulator <chemin.apk>` — nécessite $ANDROID_HOME (SDK).
# 1. Vérifie qu'un AVD existe (le crée sinon, profil `medium_phone` par
#    défaut — x86_64 + accélération KVM, voir docs/roadmap.d/packaging-android-webview-hybrid.md
#    pour pourquoi x86_64 plutôt qu'arm64-v8a ici : accélération matérielle
#    complète sur un hôte x86_64, l'émulation arm64 tournerait en traduction
#    logicielle pure, beaucoup plus lente).
# 2. Vérifie qu'un émulateur tourne réellement (`adb devices`, pas seulement
#    `emulator list` dont la colonne Status ne reflète pas l'état réel) — le
#    démarre sinon (`emulator start` ne rend la main qu'une fois prêt).
# 3. Si l'app (même nom de package que l'APK donné) est déjà installée, la
#    désinstalle d'abord — `adb uninstall` supprime aussi ses données/cache,
#    pas seulement le paquet, pour repartir d'un état propre à chaque test.
# 4. Installe l'APK puis lance son Activity de lancement (sans avoir besoin
#    de connaître son nom exact — `monkey -c android.intent.category.LAUNCHER`
#    est le mécanisme standard pour ça).
ANDROID_AVD           ?= medium_phone
ANDROID_BUILD_TOOLS   := 36.0.0

android-simulator:
	@if [ -z "$(_TARGET)" ]; then \
	    echo "Usage: make android-simulator <chemin.apk> [ANDROID_SERIAL=<serial adb>]"; exit 1; \
	fi
	@if [ -z "$$ANDROID_HOME" ]; then \
	    echo "ANDROID_HOME non défini (racine du SDK Android requise)"; exit 1; \
	fi
	@APK="$(_TARGET)"; \
	ADB="$$ANDROID_HOME/platform-tools/adb"; \
	ANDROID_TOOL="$$ANDROID_HOME/cmdline-tools/latest/bin/android"; \
	AAPT="$$ANDROID_HOME/build-tools/$(ANDROID_BUILD_TOOLS)/aapt"; \
	if [ ! -f "$$APK" ]; then echo "APK introuvable: $$APK"; exit 1; fi; \
	if [ -n "$$ANDROID_SERIAL" ]; then \
	    echo "== Cible forcée : $$ANDROID_SERIAL (ANDROID_SERIAL) =="; \
	    SERIAL="$$ANDROID_SERIAL"; \
	    if ! "$$ADB" devices | grep -q "^$$SERIAL[[:space:]]"; then \
	        echo "Appareil '$$SERIAL' non vu par adb (branché ? déverrouillé ? débogage USB autorisé ?)"; exit 1; \
	    fi; \
	else \
	    echo "== Vérification de l'AVD '$(ANDROID_AVD)' =="; \
	    if ! "$$ANDROID_TOOL" emulator list | grep -qx "$(ANDROID_AVD)"; then \
	        echo "AVD '$(ANDROID_AVD)' introuvable — création (x86_64, accélération KVM)..."; \
	        "$$ANDROID_TOOL" emulator create $(ANDROID_AVD) || exit 1; \
	    fi; \
	    SERIAL=$$("$$ADB" devices | awk '/^emulator-/{print $$1; exit}'); \
	    if [ -z "$$SERIAL" ]; then \
	        echo "Aucun émulateur démarré — démarrage de '$(ANDROID_AVD)' (fenêtre visible)..."; \
	        "$$ANDROID_TOOL" emulator start $(ANDROID_AVD) || exit 1; \
	        SERIAL=$$("$$ADB" devices | awk '/^emulator-/{print $$1; exit}'); \
	    fi; \
	    if [ -z "$$SERIAL" ]; then echo "Impossible de trouver un émulateur démarré"; exit 1; fi; \
	fi; \
	echo "Appareil : $$SERIAL"; \
	PACKAGE=$$("$$AAPT" dump badging "$$APK" | sed -n "s/^package: name='\([^']*\)'.*/\1/p"); \
	if [ -z "$$PACKAGE" ]; then echo "Impossible de déterminer le nom de package de $$APK"; exit 1; fi; \
	echo "Package : $$PACKAGE"; \
	if "$$ADB" -s "$$SERIAL" shell pm list packages | grep -q "package:$$PACKAGE$$"; then \
	    echo "Déjà installé — désinstallation (purge données/cache)..."; \
	    "$$ADB" -s "$$SERIAL" uninstall "$$PACKAGE" || true; \
	fi; \
	echo "Installation de $$APK..."; \
	"$$ADB" -s "$$SERIAL" install "$$APK" || exit 1; \
	echo "Lancement..."; \
	"$$ADB" -s "$$SERIAL" shell monkey -p "$$PACKAGE" -c android.intent.category.LAUNCHER 1

# ── Tests unitaires Cargo ─────────────────────────────────────────────────────
tests:
	RUSTFLAGS="-D warnings" cargo test

# ── Régression ────────────────────────────────────────────────────────────────
regression:
	@if [ -n "$(_TARGET)" ]; then \
	    ./ci/regression.sh $(_TARGET); \
	else \
	    ./ci/regression.sh; \
	    ./ci/unittests.sh examples/project/tests; \
	    ./ci/unittests.sh examples/tests; \
	fi

# ── CI Pipeline ───────────────────────────────────────────────────────────────
ci:
	@echo "══════════════════════════════════════════════"
	@echo " Pipeline CI — Intégration Continue"
	@echo "══════════════════════════════════════════════"
	./ci/start.sh

# ── Outils (ocaracs + ocaraunit) ─────────────────────────────────────────────
build-tools:
	RUSTFLAGS="-D warnings" cargo build --release -p ocaracs
	RUSTFLAGS="-D warnings" cargo build --release -p ocaraunit

build-tools-dev:
	RUSTFLAGS="-D warnings" cargo build -p ocaracs
	RUSTFLAGS="-D warnings" cargo build -p ocaraunit

lint-examples:
	@echo "══════════════════════════════════════════════"
	@echo " Lint ocaracs — examples/"
	@echo "══════════════════════════════════════════════"
	./target/release/ocaracs examples/ || true

tests-examples:
	@echo "══════════════════════════════════════════════"
	@echo " ocaraunit"
	@echo "══════════════════════════════════════════════"
	@if [ -n "$(_TARGET)" ]; then \
	    ./target/release/ocaraunit $(_TARGET); \
	else \
	    ./target/release/ocaraunit examples/tests; \
	    ./target/release/ocaraunit examples/project/tests; \
	fi

build-all: build build-tools
	@echo "$(GREEN)Compilation de Ocara et des outils terminée.$(RESET)"

build-all-dev: build-dev build-tools-dev
	@echo "$(GREEN)Compilation de Ocara et des outils terminée.$(RESET)"

# ── Installation ──────────────────────────────────────────────────────────────
install: clean build clean
	install -m 755 $(OCARA) /usr/local/bin/ocara
	@echo "$(GREEN)Ocara installé dans /usr/local/bin/ocara$(RESET)"
	@echo "$(GREEN)Le runtime est embarqué dans le binaire — aucun fichier supplémentaire requis.$(RESET)"

install-tools: clean-tools build-tools clean-tools
	install -m 755 ./target/release/ocaracs /usr/local/bin/ocaracs
	@echo "$(GREEN)ocaracs installé dans /usr/local/bin/ocaracs$(RESET)"
	install -m 755 ./target/release/ocaraunit /usr/local/bin/ocaraunit
	@echo "$(GREEN)ocaraunit installé dans /usr/local/bin/ocaraunit$(RESET)"

install-all: install install-tools
	@echo "$(GREEN)Ocara et tous les outils installés dans /usr/local/bin/$(RESET)"

# ── Désinstallation ─────────────────────────────────────────────────────────
uninstall:
	rm -f /usr/local/bin/ocara
	@echo "$(GREEN)Ocara désinstallé.$(RESET)"

uninstall-tools:
	rm -f /usr/local/bin/ocaracs /usr/local/bin/ocaraunit
	@echo "$(GREEN)ocaracs et ocaraunit désinstallés.$(RESET)"

uninstall-all: uninstall uninstall-tools
	@echo "$(GREEN)Ocara et tous les outils désinstallés.$(RESET)"

# ── Nettoyage ───────────────────────────────────────────────────────────────
clean:
	cargo clean -p ocara -p ocara_runtime -p ocara_runtime_tauri -p ocara_runtime_sdl
	rm -f $(TMP)

clean-tools:
	cargo clean -p ocaracs -p ocaraunit
	rm -rf .ocaraunit_cache

clean-all: clean clean-tools
	rm -f *.o *.a