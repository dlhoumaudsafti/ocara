# examples/advanced/mini_project_ui_hexa/Makefile — build desktop/Android en une commande.
#
#   make desktop            Compile pour l'hôte et lance (fenêtre native, ocara.UI)
#   make preview            Aperçu de l'interface dans un navigateur (sans programme)
#   make android            Compile arm64 + x86_64, les deux dans le MÊME APK debug
#   make android-production Idem, mais release signé (production) — voir docs/android.md
#   make simulator          Installe+lance l'APK DEBUG sur un appareil détecté ou l'émulateur
#   make tests              Tests unitaires (tests/, même arborescence que le projet)
#
# Même découpage que mini_project_hexa ; ici, ni serveur ni WebView : sur
# Android, l'interface est rendue en Compose par l'app de packaging/android
# (libocara_runtime_android_ui.a, `make build-android-features`).
#
# ANDROID_NDK_HOME/ANDROID_HOME par défaut sur ce poste — surchageables via
# l'environnement si besoin (ex: ANDROID_NDK_HOME=/autre/chemin make android).

ANDROID_NDK_HOME ?= $(HOME)/android-ndk-r30
ANDROID_HOME     ?= $(HOME)/android-sdk

ROOT       := ../../..
OCARA      := $(ROOT)/target/release/ocara
PACKAGING  := $(ROOT)/packaging/android
ASSETS     := $(PACKAGING)/app/src/main/assets
# chemins ABSOLUS : passés tels quels au Makefile racine via `-C`, qui change
# de répertoire avant de les lire (un chemin relatif à CE dossier serait alors faux).
APK_DEBUG    := $(abspath $(PACKAGING)/app/build/outputs/apk/debug/app-debug.apk)
APK_RELEASE  := $(abspath $(PACKAGING)/app/build/outputs/apk/release/app-release.apk)

.PHONY: desktop preview android android-production simulator tests _android_native_libs _android_assets clean

desktop:
	$(OCARA) main.oc -o out
	./out

preview:
	$(OCARA) ui/garage.oc.ui --preview -o preview.html
	@echo "Aperçu : file://$(abspath preview.html)"

# Runtime, pont JNI et backend Compose d'ocara.UI (arm64 ET x86_64), puis le
# `.so` de CET exemple pour les deux ABI — partagé par `android` (debug) et
# `android-production` (release).
_android_native_libs:
	$(MAKE) -C $(ROOT) ANDROID_NDK_HOME="$(ANDROID_NDK_HOME)" build-runtime-android build-jni-bridge-android build-android-features
	$(MAKE) -C $(ROOT) ANDROID_NDK_HOME="$(ANDROID_NDK_HOME)" ANDROID_TARGET=x86_64-linux-android build-runtime-android build-jni-bridge-android build-android-features
	$(OCARA) main.oc --target aarch64-linux-android \
		--android-runtime $(ROOT)/target/aarch64-linux-android/release/libocara_runtime.a \
		--android-jni-bridge $(ROOT)/target/aarch64-linux-android/release/libocara_runtime_android_jni.a \
		--android-ndk "$(ANDROID_NDK_HOME)" \
		-o $(PACKAGING)/app/src/main/jniLibs/arm64-v8a/libmain.so
	$(OCARA) main.oc --target x86_64-linux-android \
		--android-runtime $(ROOT)/target/x86_64-linux-android/release/libocara_runtime.a \
		--android-jni-bridge $(ROOT)/target/x86_64-linux-android/release/libocara_runtime_android_jni.a \
		--android-ndk "$(ANDROID_NDK_HOME)" \
		-o $(PACKAGING)/app/src/main/jniLibs/x86_64/libmain.so

# Textes de l'interface (languages/<langue>/ui.yaml), lus à l'exécution :
# copiés dans les assets, que l'app recopie dans le dossier du programme.
_android_assets:
	rm -rf $(ASSETS)/languages
	mkdir -p $(ASSETS)
	cp -r languages $(ASSETS)/languages

android: _android_native_libs _android_assets
	cd $(PACKAGING) && ./gradlew assembleDebug

android-production: _android_native_libs _android_assets
	@if [ -z "$$OCARA_RELEASE_KEYSTORE" ] || [ -z "$$OCARA_RELEASE_KEYSTORE_PASSWORD" ] || [ -z "$$OCARA_RELEASE_KEY_ALIAS" ] || [ -z "$$OCARA_RELEASE_KEY_PASSWORD" ]; then \
		echo "OCARA_RELEASE_KEYSTORE, OCARA_RELEASE_KEYSTORE_PASSWORD, OCARA_RELEASE_KEY_ALIAS et OCARA_RELEASE_KEY_PASSWORD sont requis — voir docs/android.md pour générer un keystore et les définir."; \
		exit 1; \
	fi
	@if [ ! -f "$$OCARA_RELEASE_KEYSTORE" ]; then \
		echo "Keystore introuvable : $$OCARA_RELEASE_KEYSTORE"; \
		exit 1; \
	fi
	cd $(PACKAGING) && ./gradlew assembleRelease
	@echo "APK de production : $(APK_RELEASE)"

simulator:
	@if [ ! -f "$(APK_DEBUG)" ]; then echo "APK introuvable ($(APK_DEBUG)) — lancer 'make android' d'abord."; exit 1; fi
	@ADB="$(ANDROID_HOME)/platform-tools/adb"; \
	echo "0) Émulateur (créé/démarré automatiquement si besoin)"; \
	"$$ADB" devices | awk 'NR>1 && NF==2 {n++; print n") " $$1}'; \
	printf "Choix : "; read CHOICE; \
	if [ "$$CHOICE" = "0" ]; then \
		$(MAKE) -C $(ROOT) ANDROID_HOME="$(ANDROID_HOME)" android-simulator $(APK_DEBUG); \
	else \
		SERIAL=$$("$$ADB" devices | awk -v n="$$CHOICE" 'NR>1 && NF==2 {c++; if (c==n) print $$1}'); \
		if [ -z "$$SERIAL" ]; then echo "Choix invalide."; exit 1; fi; \
		$(MAKE) -C $(ROOT) ANDROID_HOME="$(ANDROID_HOME)" ANDROID_SERIAL=$$SERIAL android-simulator $(APK_DEBUG); \
	fi

# Tests unitaires : tests/ reprend l'arborescence du projet (context/<contexte>/
# <couche>/…). Ils tournent sur ./app.db sans toucher aux données existantes
# (voitures TEST-OCUNIT-* créées puis supprimées, voir tests/support/Fixture.oc) ;
# les contrôleurs agissent sur une interface créée sans être affichée.
tests:
	$(ROOT)/target/release/ocaraunit --coverage=. tests
	$(ROOT)/target/release/ocaraunit --line-coverage=. tests

clean:
	rm -rf out out.o app.db uploads preview.html
	rm -f $(PACKAGING)/app/src/main/jniLibs/arm64-v8a/libmain.so
	rm -f $(PACKAGING)/app/src/main/jniLibs/x86_64/libmain.so
