# examples/advanced/mini_project/Makefile — build desktop/Android en une commande.
#
#   make desktop    Compile pour l'hôte et lance (fenêtre Tauri)
#   make android    Compile arm64 + x86_64, les deux dans le MÊME APK, construit l'APK
#   make simulator  Installe+lance l'APK sur un appareil détecté ou l'émulateur (avec choix)
#
# ANDROID_NDK_HOME/ANDROID_HOME par défaut sur ce poste — surchageables via
# l'environnement si besoin (ex: ANDROID_NDK_HOME=/autre/chemin make android).

ANDROID_NDK_HOME ?= /home/david.lhoumaud@local/android-ndk-r30
ANDROID_HOME     ?= /home/david.lhoumaud@local/android-sdk

ROOT       := ../../..
OCARA      := $(ROOT)/target/release/ocara
PACKAGING  := $(ROOT)/packaging/android
# chemin ABSOLU : passé tel quel au Makefile racine via `-C`, qui change de
# répertoire avant de le lire (un chemin relatif à CE dossier serait alors faux).
APK        := $(abspath $(PACKAGING)/app/build/outputs/apk/debug/app-debug.apk)

.PHONY: desktop android simulator clean

desktop:
	$(OCARA) build main.oc -o out
	./out

android:
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
	cd $(PACKAGING) && ./gradlew assembleDebug

simulator:
	@if [ ! -f "$(APK)" ]; then echo "APK introuvable ($(APK)) — lancer 'make android' d'abord."; exit 1; fi
	@ADB="$(ANDROID_HOME)/platform-tools/adb"; \
	echo "0) Émulateur (créé/démarré automatiquement si besoin)"; \
	"$$ADB" devices | awk 'NR>1 && NF==2 {n++; print n") " $$1}'; \
	printf "Choix : "; read CHOICE; \
	if [ "$$CHOICE" = "0" ]; then \
		$(MAKE) -C $(ROOT) ANDROID_HOME="$(ANDROID_HOME)" android-simulator $(APK); \
	else \
		SERIAL=$$("$$ADB" devices | awk -v n="$$CHOICE" 'NR>1 && NF==2 {c++; if (c==n) print $$1}'); \
		if [ -z "$$SERIAL" ]; then echo "Choix invalide."; exit 1; fi; \
		$(MAKE) -C $(ROOT) ANDROID_HOME="$(ANDROID_HOME)" ANDROID_SERIAL=$$SERIAL android-simulator $(APK); \
	fi

clean:
	rm -f out out.o app.db
	rm -f $(PACKAGING)/app/src/main/jniLibs/arm64-v8a/libmain.so
	rm -f $(PACKAGING)/app/src/main/jniLibs/x86_64/libmain.so
