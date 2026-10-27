package com.ocara.bridge

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicInteger

/**
 * `ocara.AndroidPermissions`. Une permission doit aussi être déclarée dans
 * `AndroidManifest.xml`, sinon Android la refuse sans rien demander.
 */
object OcaraPermissions {
    private val aliases = mapOf(
        "camera" to Manifest.permission.CAMERA,
        "microphone" to Manifest.permission.RECORD_AUDIO,
        "notifications" to "android.permission.POST_NOTIFICATIONS",
        "location" to Manifest.permission.ACCESS_FINE_LOCATION,
        "coarse_location" to Manifest.permission.ACCESS_COARSE_LOCATION,
        "contacts" to Manifest.permission.READ_CONTACTS,
        "activity_recognition" to "android.permission.ACTIVITY_RECOGNITION",
    )
    private val pending = ConcurrentHashMap<Int, CompletableFuture<Boolean>>()
    private val nextCode = AtomicInteger(0)

    /** Nom court (`camera`…) ou nom Android complet (`android.permission.CAMERA`). */
    fun resolve(name: String): String =
        aliases[name] ?: name.takeIf { '.' in it }
            ?: throw OcaraAndroidError(OcaraAndroidError.ARGUMENT, "permission inconnue : '$name' (noms courts : ${aliases.keys.joinToString()})")

    @JvmStatic
    fun isGranted(name: String): Boolean {
        val permission = resolve(name)
        // Avant Android 13, les notifications ne demandent aucune permission.
        if (permission == "android.permission.POST_NOTIFICATIONS" && Build.VERSION.SDK_INT < 33) return true
        return OcaraBridge.context.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED
    }

    /** Demande la permission et attend la réponse de l'utilisateur. */
    @JvmStatic
    fun request(name: String): Boolean {
        if (isGranted(name)) return true
        requireBackgroundThread("AndroidPermissions::request")
        val activity = OcaraBridge.activity
            ?: throw OcaraAndroidError(OcaraAndroidError.UNAVAILABLE, "aucune Activity au premier plan (OcaraBridge.attach)")
        val code = 1000 + (nextCode.incrementAndGet() and 0xfff)
        val answer = CompletableFuture<Boolean>()
        pending[code] = answer
        activity.runOnUiThread { activity.requestPermissions(arrayOf(resolve(name)), code) }
        return try { answer.get() } finally { pending.remove(code) }
    }

    /** À appeler depuis `Activity.onRequestPermissionsResult`. */
    @JvmStatic
    fun onResult(requestCode: Int, grantResults: IntArray) {
        pending[requestCode]?.complete(grantResults.isNotEmpty() && grantResults.all { it == PackageManager.PERMISSION_GRANTED })
    }
}
