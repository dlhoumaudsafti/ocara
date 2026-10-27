package com.ocara.bridge

import android.app.Activity
import android.content.Context
import java.io.File

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
    private var appContext: Context? = null

    /** Activity au premier plan, pour les demandes de permission ([OcaraPermissions]). */
    @Volatile
    var activity: Activity? = null
        private set

    /** Contexte de l'app, pour les builtins `ocara.Android*` (classes `Ocara*` de ce package). */
    val context: Context
        get() = appContext ?: throw OcaraAndroidError(OcaraAndroidError.UNAVAILABLE, "OcaraBridge.attach(activity) n'a pas été appelé")

    /** À appeler dans `onCreate`, avant [nativeStartServer]. */
    fun attach(activity: Activity) {
        appContext = activity.applicationContext
        this.activity = activity
    }

    /** À appeler dans `onDestroy`. */
    fun detach(activity: Activity) {
        if (this.activity === activity) this.activity = null
    }

    /**
     * Fichier d'un chemin Ocara : un chemin relatif part du répertoire de
     * travail du programme ([nativeStartServer] y fait `chdir`), pas de celui
     * de la JVM.
     */
    fun file(path: String): File = File(path).let { if (it.isAbsolute) it else File(context.filesDir, path) }

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
     *
     * @param dataDir `Context.getFilesDir().getAbsolutePath()` — PAS un chemin
     *   construit à la main : `getFilesDir()` est la seule API qui garantit la
     *   création réelle du répertoire ET son étiquetage SELinux correct par le
     *   framework Android (une reproduction sur un vrai appareil a montré
     *   qu'un chemin "à la main", même dans le bac à sable de l'app, pouvait
     *   échouer à l'ouverture SQLite là où `getFilesDir()` fonctionne — voir
     *   docs/roadmap.d/runtime-android-exit-unsafe.md). Le pont fait `chdir()`
     *   vers ce répertoire avant de démarrer le programme Ocara : tout chemin
     *   relatif utilisé côté Ocara (`"./app.db"`, `"./public/"`) y pointe
     *   alors, comme sur desktop où le répertoire de travail est celui depuis
     *   lequel on lance le programme.
     */
    external fun nativeStartServer(dataDir: String): Boolean
}
