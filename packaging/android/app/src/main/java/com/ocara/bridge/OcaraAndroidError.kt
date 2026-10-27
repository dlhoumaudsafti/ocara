package com.ocara.bridge

/**
 * Erreur levée côté Ocara en `AndroidException` avec [code] (lu par
 * runtime_android/common/src/java.rs). Toute autre exception Java devient le
 * code 105 (101 pour une `SecurityException`).
 */
class OcaraAndroidError(val code: Int, message: String) : RuntimeException(message) {
    companion object {
        const val UNAVAILABLE = 100
        const val PERMISSION = 101
        const val ARGUMENT = 102
        const val DEVICE = 103
        const val IO = 104
        const val TIMEOUT = 106
        const val STATE = 107
    }
}

/** Refuse un appel bloquant sur le thread principal Android. */
internal fun requireBackgroundThread(what: String) {
    if (android.os.Looper.myLooper() == android.os.Looper.getMainLooper()) {
        throw OcaraAndroidError(OcaraAndroidError.STATE, "$what bloque : interdit sur le thread principal Android")
    }
}
