# examples/advanced/mini_project/Makefile — build desktop/Android en une commande.
#
#   make desktop            Compile pour l'hôte et lance (fenêtre Tauri)
#   make android            Compile arm64 + x86_64, les deux dans le MÊME APK debug
#   make android-production Idem, mais release signé (production) — voir docs/android.md
#   make simulator          Installe+lance l'APK DEBUG sur un appareil détecté ou l'émulateur
#
# ANDROID_NDK_HOME/ANDROID_HOME par défaut sur ce poste — surchageables via
# l'environnement si besoin (ex: ANDROID_NDK_HOME=/autre/chemin make android).
#
# `android-production` nécessite en plus OCARA_RELEASE_KEYSTORE et les 3
# variables associées (voir sa cible plus bas et docs/android.md — procédure
# complète, génération du keystore incluse) — jamais de secret en dur ici.

ANDROID_NDK_HOME ?= <PATH>/android-ndk-r30
ANDROID_HOME     ?= <PATH>/android-sdk

ROOT       := ../../..
OCARA      := $(ROOT)/target/release/ocara
PACKAGING  := $(ROOT)/packaging/android
# chemins ABSOLUS : passés tels quels au Makefile racine via `-C`, qui change
# de répertoire avant de les lire (un chemin relatif à CE dossier serait alors faux).
APK_DEBUG    := $(abspath $(PACKAGING)/app/build/outputs/apk/debug/app-debug.apk)
APK_RELEASE  := $(abspath $(PACKAGING)/app/build/outputs/apk/release/app-release.apk)

.PHONY: desktop android android-production simulator _android_native_libs clean

desktop:
	$(OCARA) build main.oc -o out
	./out

# Runtime + pont JNI (arm64 ET x86_64) + le `.so` de CET exemple pour les deux
# ABI — partagé par `android` (debug) et `android-production` (release) : la
# différence entre les deux n'est QUE l'étape Gradle finale (assembleDebug vs
# assembleRelease, signature, minification), jamais ce `.so` natif lui-même.
_android_native_libs:
	$(MAKE) -C $(ROOT) ANDROID_NDK_HOME="$(ANDROID_NDK_HOME)" build-runtime-android build-jni-bridge-android
	$(MAKE) -C $(ROOT) ANDROID_NDK_HOME="$(ANDROID_NDK_HOME)" ANDROID_TARGET=x86_64-linux-android build-runtime-android build-jni-bridge-android
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

android: _android_native_libs
	cd $(PACKAGING) && ./gradlew assembleDebug

# APK de production : release, signé avec un VRAI keystore (jamais celui de
# debug, auto-généré et impossible à faire reconnaître comme mise à jour
# légitime par le Play Store ou par un appareil ayant déjà l'app installée),
# minifié/réduit par R8 (voir app/build.gradle.kts + app/proguard-rules.pro —
# ce dernier garde explicitement OcaraBridge.nativeStartServer, dont le nom
# Java exact est résolu en dur par le pont JNI natif : le renommer casserait
# le chargement au premier appel). Procédure complète (génération du
# keystore) : docs/android.md.
android-production: _android_native_libs
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

clean:
	rm -f out out.o app.db
	rm -f $(PACKAGING)/app/src/main/jniLibs/arm64-v8a/libmain.so
	rm -f $(PACKAGING)/app/src/main/jniLibs/x86_64/libmain.so
