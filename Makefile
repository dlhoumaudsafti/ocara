OCARA   := ./target/release/ocara
TMP     := /tmp/oc_regression
GREEN   := \033[0;32m
RED     := \033[0;31m
RESET   := \033[0m

# Argument optionnel : make regression builtins/io
_TARGET := $(filter-out all build build-tools test regression lint clean help install install-tools uninstall,$(MAKECMDGOALS))

.PHONY: all build build-tools test regression lint clean help install install-tools uninstall unittest $(_TARGET)

all: build test regression

# ── Aide ──────────────────────────────────────────────────────────────────────
help:
	@echo "Usage : make <cible> [argument]"
	@echo ""
	@echo "  build                   Compile le runtime + le compilateur Ocara"
	@echo "  build-tools             Compile les outils (ocaracs, ocaraunit)"
	@echo "  test                    Lance les tests unitaires Cargo (cargo test)"
	@echo "  regression              Lance la régression complète (tous les exemples)"
	@echo "  regression <chemin>     Lance uniquement examples/<chemin>.oc"
	@echo "                            ex: make regression builtins/io"
	@echo "                            ex: make regression 07_loops"
	@echo "                            ex: make regression project/main"
	@echo "  lint                    Lance ocaracs sur tous les exemples"
	@echo "  unittest [<dossier>]    Lance ocaraunit sur le dossier (défaut: .)"
	@echo "  all                     build + test + regression"
	@echo "  clean                   Supprime les artefacts de compilation"
	@echo "  install                 Installe ocara dans /usr/local/bin/"
	@echo "  install-tools           Installe ocaracs dans /usr/local/bin/"
	@echo "  uninstall               Supprime ocara de /usr/local/bin/"
	@echo "  help                    Affiche ce message"

# Absorber l'argument positionnel pour éviter "No rule to make target"
ifneq ($(_TARGET),)
$(_TARGET):
	@:
endif

# ── Compilation du compilateur + runtime ─────────────────────────────────────
# Le runtime doit être compilé en premier : build.rs l'embarque dans le binaire
build:
	RUSTFLAGS="-D warnings" cargo build --release -p ocara_runtime
	RUSTFLAGS="-D warnings" cargo build --release -p ocara
	RUSTFLAGS="-D warnings" cargo build --release -p ocaracs
	RUSTFLAGS="-D warnings" cargo build --release -p ocaraunit

# ── Tests unitaires Cargo ─────────────────────────────────────────────────────
test:
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

clean:
	cargo clean
	rm -f $(TMP)

# ── Outils (ocaracs + ocaraunit) ─────────────────────────────────────────────
build-tools:
	RUSTFLAGS="-D warnings" cargo build --release -p ocaracs
	RUSTFLAGS="-D warnings" cargo build --release -p ocaraunit

lint: build-tools
	@echo "══════════════════════════════════════════════"
	@echo " Lint ocaracs — examples/"
	@echo "══════════════════════════════════════════════"
	./target/release/ocaracs examples/ || true

unittest: build-tools build
	@echo "══════════════════════════════════════════════"
	@echo " ocaraunit"
	@echo "══════════════════════════════════════════════"
	OCARA=./target/release/ocara ./target/release/ocaraunit $(if $(_TARGET),$(_TARGET),.)

# ── Installation ──────────────────────────────────────────────────────────────
install: build
	install -m 755 $(OCARA) /usr/local/bin/ocara
	@echo "$(GREEN)Ocara installé dans /usr/local/bin/ocara$(RESET)"
	@echo "$(GREEN)Le runtime est embarqué dans le binaire — aucun fichier supplémentaire requis.$(RESET)"

install-tools: build-tools
	install -m 755 ./target/release/ocaracs /usr/local/bin/ocaracs
	@echo "$(GREEN)ocaracs installé dans /usr/local/bin/ocaracs$(RESET)"
	install -m 755 ./target/release/ocaraunit /usr/local/bin/ocaraunit
	@echo "$(GREEN)ocaraunit installé dans /usr/local/bin/ocaraunit$(RESET)"

uninstall:
	rm -f /usr/local/bin/ocara /usr/local/bin/ocaracs /usr/local/bin/ocaraunit
	@echo "$(GREEN)Ocara, ocaracs et ocaraunit désinstallés.$(RESET)"

