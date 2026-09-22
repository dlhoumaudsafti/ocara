OCARA   := ./target/release/ocara
TMP     := /tmp/oc_regression
GREEN   := \033[0;32m
RED     := \033[0;31m
RESET   := \033[0m

# Argument optionnel : make regression builtins/io
_TARGET := $(filter-out build build-dev build-tools build-tools-dev build-all build-all-dev pkgconfig-shim test tests regression lint-examples tests-examples clean clean-tools clean-all help install install-tools install-all uninstall uninstall-tools uninstall-all build-runtime-android build-runtime-sdl-android build-jni-bridge-android,$(MAKECMDGOALS))

.PHONY: build build-dev build-tools build-tools-dev build-all build-all-dev pkgconfig-shim test tests regression ci lint-examples tests-examples clean clean-tools clean-all help install install-tools install-all uninstall uninstall-tools uninstall-all build-runtime-android build-runtime-sdl-android build-jni-bridge-android $(_TARGET)

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
ANDROID_API   := 24
ANDROID_NDK_CLANG_DIR := $(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/linux-x86_64/bin
ANDROID_CC      := $(ANDROID_NDK_CLANG_DIR)/aarch64-linux-android$(ANDROID_API)-clang
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
	CC_aarch64_linux_android="$(ANDROID_CC)" \
	AR_aarch64_linux_android="$(ANDROID_AR)" \
	RANLIB_aarch64_linux_android="$(ANDROID_RANLIB)" \
	CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$(ANDROID_CC)" \
	RUSTFLAGS="-D warnings" cargo build --release -p ocara_runtime --target aarch64-linux-android -j4

# `ocara_runtime_android_jni` (packaging-android-webview-hybrid.md) : pont JNI
# générique, indépendant de tout import Ocara — voir packaging/android/README.md
# pour l'usage complet (liaison d'un .so chargeable par une Activity Android).
build-jni-bridge-android:
	@if [ -z "$(ANDROID_NDK_HOME)" ]; then \
	    echo "ANDROID_NDK_HOME non défini (racine du NDK Android requise)"; exit 1; \
	fi
	CC_aarch64_linux_android="$(ANDROID_CC)" \
	AR_aarch64_linux_android="$(ANDROID_AR)" \
	RANLIB_aarch64_linux_android="$(ANDROID_RANLIB)" \
	CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$(ANDROID_CC)" \
	RUSTFLAGS="-D warnings" cargo build --release -p ocara_runtime_android_jni --target aarch64-linux-android -j4

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
	CC_aarch64_linux_android="$(ANDROID_CC)" \
	CXX_aarch64_linux_android="$(ANDROID_CC)++" \
	AR_aarch64_linux_android="$(ANDROID_AR)" \
	RANLIB_aarch64_linux_android="$(ANDROID_RANLIB)" \
	CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$(ANDROID_CC)" \
	CMAKE_TOOLCHAIN_FILE="$(ANDROID_NDK_HOME)/build/cmake/android.toolchain.cmake" \
	RUSTFLAGS="-D warnings" cargo build --release -p ocara_runtime_sdl --target aarch64-linux-android -j1

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