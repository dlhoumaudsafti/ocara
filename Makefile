OCARA   := ./target/release/ocara
TMP     := /tmp/oc_regression
GREEN   := \033[0;32m
RED     := \033[0;31m
RESET   := \033[0m

# Argument optionnel : make regression builtins/io
_TARGET := $(filter-out build build-dev build-tools build-tools-dev build-all build-all-dev pkgconfig-shim test tests regression lint-examples tests-examples clean clean-tools clean-all help install install-tools install-all uninstall uninstall-tools uninstall-all,$(MAKECMDGOALS))

.PHONY: build build-dev build-tools build-tools-dev build-all build-all-dev pkgconfig-shim test tests regression ci lint-examples tests-examples clean clean-tools clean-all help install install-tools install-all uninstall uninstall-tools uninstall-all $(_TARGET)

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

# ── Compilation du compilateur + runtime ─────────────────────────────────────
# Le runtime doit être compilé en premier : build.rs l'embarque dans le binaire
# -j1 sur ocara : Cranelift est très lourd à compiler en parallèle (SIGKILL OOM)
build: pkgconfig-shim
	PKG_CONFIG_PATH="$(PKGCONFIG_SHIM):$$PKG_CONFIG_PATH" RUSTFLAGS="-D warnings" cargo build --release -p ocara_runtime -j4
	RUSTFLAGS="-D warnings" cargo build --release -p ocara -j4

build-dev: pkgconfig-shim
	PKG_CONFIG_PATH="$(PKGCONFIG_SHIM):$$PKG_CONFIG_PATH" RUSTFLAGS="-D warnings" cargo build -p ocara_runtime -j4
	RUSTFLAGS="-D warnings" cargo build -p ocara -j4

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
	cargo clean -p ocara -p ocara_runtime
	rm -f $(TMP)

clean-tools:
	cargo clean -p ocaracs -p ocaraunit
	rm -rf .ocaraunit_cache

clean-all: clean clean-tools
	rm -f *.o *.a