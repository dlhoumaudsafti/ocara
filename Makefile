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

# ── Tests unitaires Cargo ─────────────────────────────────────────────────────
test:
	RUSTFLAGS="-D warnings" cargo test

# ── Régression ────────────────────────────────────────────────────────────────
regression: build
	@if [ -n "$(_TARGET)" ]; then \
	    src=examples/$(_TARGET).oc; \
	    name=$(_TARGET); \
	    echo "══════════════════════════════════════════════"; \
	    echo " Régression $$src"; \
	    echo "══════════════════════════════════════════════"; \
	    case "$$name" in \
	        21_errors) \
	            $(OCARA) $$src --check; rc=$$?; \
	            if [ $$rc -eq 0 ]; then \
	                echo "$(RED)FAIL [check devait échouer] $$name$(RESET)"; rm -f $(TMP); exit 1; \
	            fi; \
	            echo "$(GREEN)OK   $$name$(RESET)" ;; \
	        *) \
	            $(OCARA) $$src -o $(TMP); rc=$$?; \
	            if [ $$rc -ne 0 ]; then \
	                echo "$(RED)FAIL [compile] $$name$(RESET)"; rm -f $(TMP); exit 1; \
	            fi; \
	            case "$$name" in \
	                03_builtins) echo -e "david\n45" | $(TMP) ;; \
	                builtins/io) printf 'Alice\nParis\n21\n3.14\ntrue\nrust,ocara,web\nlang=fr,theme=dark\n' | $(TMP) ;; \
	                builtins/http) $(TMP) > /dev/null ;; \
	                builtins/httpserver) examples/builtins/httpserver.sh $(TMP) ;; \
	                *) $(TMP) ;; \
	            esac; \
	            if [ $$? -ne 0 ]; then \
	                echo "$(RED)FAIL [run] $$name$(RESET)"; rm -f $(TMP); exit 1; \
	            fi; \
	            echo "$(GREEN)OK   $$name$(RESET)"; \
	            rm -f $(TMP) ;; \
	    esac; \
	else \
	    fail=0; \
	    failed=""; \
	    echo "══════════════════════════════════════════════"; \
	    echo " Régression examples/NN_*.oc"; \
	    echo "══════════════════════════════════════════════"; \
	    for src in examples/[0-9][0-9]_*.oc; do \
	        name=$$(basename $$src .oc); \
	        if [ "$$name" = "21_errors" ]; then \
	            $(OCARA) $$src --check > /dev/null; rc=$$?; \
	            if [ $$rc -eq 0 ]; then \
	                echo "$(RED)FAIL [check devait échouer] $$name$(RESET)"; fail=1; failed="$$failed $$name"; \
	            else \
	                echo "$(GREEN)OK   $$name$(RESET)"; \
	            fi; \
	        else \
	            $(OCARA) $$src -o $(TMP); rc=$$?; \
	            if [ $$rc -ne 0 ]; then \
	                echo "$(RED)FAIL [compile] $$name$(RESET)"; fail=1; failed="$$failed $$name"; \
	            else \
	                case "$$name" in \
	                    03_builtins) echo -e "david\n45" | $(TMP) ;; \
	                    *) $(TMP) ;; \
	                esac; \
	                if [ $$? -ne 0 ]; then \
	                    echo "$(RED)FAIL [run] $$name$(RESET)"; fail=1; failed="$$failed $$name"; \
	                else \
	                    echo "$(GREEN)OK   $$name$(RESET)"; \
	                fi; \
	            fi; \
	        fi; \
	    done; \
	    echo ""; \
	    echo "══════════════════════════════════════════════"; \
	    echo " Régression examples/project/main.oc"; \
	    echo "══════════════════════════════════════════════"; \
	    $(OCARA) examples/project/main.oc -o $(TMP); rc=$$?; \
	    if [ $$rc -ne 0 ]; then \
	        echo "$(RED)FAIL [compile] project/main.oc$(RESET)"; fail=1; failed="$$failed project/main"; \
	    else \
	        $(TMP); \
	        if [ $$? -ne 0 ]; then \
	            echo "$(RED)FAIL [run] project/main.oc$(RESET)"; fail=1; failed="$$failed project/main"; \
	        else \
	            echo "$(GREEN)OK   project/main.oc$(RESET)"; \
	        fi; \
	    fi; \
	    echo ""; \
	    echo "══════════════════════════════════════════════"; \
	    echo " Régression examples/builtins/*.oc"; \
	    echo "══════════════════════════════════════════════"; \
	    for src in examples/builtins/*.oc; do \
	        name=$$(basename $$src .oc); \
	        $(OCARA) $$src -o $(TMP); rc=$$?; \
	        if [ $$rc -ne 0 ]; then \
	            echo "$(RED)FAIL [compile] builtins/$$name$(RESET)"; fail=1; failed="$$failed builtins/$$name"; \
	        else \
	            case "$$name" in \
	                io) printf 'Alice\nParis\n21\n3.14\ntrue\nrust,ocara,web\nlang=fr,theme=dark\n' | $(TMP) ;; \
	                http) $(TMP) > /dev/null ;; \
	                httpserver) examples/builtins/httpserver.sh $(TMP) ;; \
	                *) $(TMP) ;; \
	            esac; \
	            if [ $$? -ne 0 ]; then \
	                echo "$(RED)FAIL [run] builtins/$$name$(RESET)"; fail=1; failed="$$failed builtins/$$name"; \
	            else \
	                echo "$(GREEN)OK   builtins/$$name$(RESET)"; \
	            fi; \
	        fi; \
	    done; \
	    rm -f $(TMP); \
	    echo ""; \
	    echo "══════════════════════════════════════════════"; \
	    if [ $$fail -eq 0 ]; then \
	        echo "$(GREEN)Tous les tests ont réussi.$(RESET)"; \
	    else \
	        echo "$(RED)Échecs :$(RESET)"; \
	        for f in $$failed; do echo "  $(RED)✗ $$f$(RESET)"; done; \
	        echo ""; \
	        echo "$(RED)Des tests ont échoué.$(RESET)"; exit 1; \
	    fi; \
	    echo "══════════════════════════════════════════════"; \
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

