# examples/advanced/android_native/Makefile — builtins ocara.Android* dans
# l'app hybride de packaging/android (même squelette que mini_project).
#
#   make android     Compile arm64 + x86_64, les deux dans le MÊME APK debug
#   make simulator   Installe et lance l'APK debug (émulateur ou appareil)
#
# ANDROID_NDK_HOME/ANDROID_HOME surchargeables par l'environnement.

ANDROID_NDK_HOME ?= $(HOME)/android-ndk-r30
ANDROID_HOME     ?= $(HOME)/android-sdk

ROOT      := ../../..
OCARA     := $(ROOT)/target/release/ocara
PACKAGING := $(ROOT)/packaging/android
APK_DEBUG := $(abspath $(PACKAGING)/app/build/outputs/apk/debug/app-debug.apk)
NATIVE    := build-runtime-android build-jni-bridge-android build-android-features

.PHONY: android simulator clean

# Runtime, pont JNI et crates runtime_android/* pour les deux ABI, puis le
# `.so` de cet exemple : `ocara` y lie les archives des builtins importés,
# trouvées à côté de --android-runtime.
android:
	$(MAKE) -C $(ROOT) ANDROID_NDK_HOME="$(ANDROID_NDK_HOME)" $(NATIVE)
	$(MAKE) -C $(ROOT) ANDROID_NDK_HOME="$(ANDROID_NDK_HOME)" ANDROID_TARGET=x86_64-linux-android $(NATIVE)
	for abi in aarch64:arm64-v8a x86_64:x86_64; do \
		arch=$${abi%%:*}; dir=$${abi##*:}; \
		$(OCARA) main.oc --target $$arch-linux-android \
			--android-runtime $(ROOT)/target/$$arch-linux-android/release/libocara_runtime.a \
			--android-jni-bridge $(ROOT)/target/$$arch-linux-android/release/libocara_runtime_android_jni.a \
			--android-ndk "$(ANDROID_NDK_HOME)" \
			-o $(PACKAGING)/app/src/main/jniLibs/$$dir/libmain.so || exit 1; \
	done
	cd $(PACKAGING) && ./gradlew assembleDebug

simulator:
	@if [ ! -f "$(APK_DEBUG)" ]; then echo "APK introuvable ($(APK_DEBUG)) — lancer 'make android' d'abord."; exit 1; fi
	$(MAKE) -C $(ROOT) ANDROID_HOME="$(ANDROID_HOME)" android-simulator $(APK_DEBUG)

clean:
	rm -f $(PACKAGING)/app/src/main/jniLibs/arm64-v8a/libmain.so
	rm -f $(PACKAGING)/app/src/main/jniLibs/x86_64/libmain.so
