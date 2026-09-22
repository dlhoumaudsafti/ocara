package com.ocara.bridge

/**
 * Pont JNI vers un programme Ocara compilé pour Android (voir
 * runtime_android_jni/src/lib.rs, et docs/roadmap.d/packaging-android-webview-hybrid.md).
 *
 * Package et nom de classe FIGÉS : `runtime_android_jni` exporte
 * `Java_com_ocara_bridge_OcaraBridge_nativeStartServer`, un symbole JNI dont
 * le nom encode ce chemin exact (convention `Java_<package>_<Classe>_<méthode>`).
 * N'importe quelle app Android peut inclure cette même classe, quel que soit
 * son propre package applicatif (ici `com.ocara.demo`) — voir la doc du crate
 * Rust pour la justification de ce choix (pas de configuration, un nom fixe).
 */
object OcaraBridge {
    init {
        // Le nom "main" correspond au `-o libmain.so` utilisé lors de la
        // liaison Android (`ocara ... --android-jni-bridge ... -o libmain.so`)
        // — `loadLibrary` retire lui-même le préfixe "lib"/suffixe ".so".
        System.loadLibrary("main")
    }

    /**
     * Démarre le programme Ocara lié dans `libmain.so` sur un thread séparé
     * (voir runtime_android_jni) et revient immédiatement — ne bloque JAMAIS
     * le thread appelant. Fire-and-forget pour cette première version : ni le
     * moment où le serveur est réellement prêt, ni son arrêt, ne sont
     * communiqués à l'appelant (voir la doc de
     * packaging-android-webview-hybrid.md, "cycle de vie Android", non résolu).
     */
    external fun nativeStartServer(): Boolean
}
