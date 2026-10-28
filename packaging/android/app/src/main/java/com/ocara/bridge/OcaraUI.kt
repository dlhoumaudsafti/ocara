package com.ocara.bridge

import android.app.AlertDialog
import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.os.Build
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.view.WindowManager
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import java.util.concurrent.CompletableFuture

/**
 * `ocara.AndroidUI` : spécificités de l'interface Android. Les événements
 * (bouton retour) remontent à Ocara par [OcaraBridge.nativeEvent].
 */
object OcaraUI {
    private var backCallback: OnBackPressedCallback? = null

    /** Réponses de boîtes en attente : rendues « non » si l'Activity disparaît. */
    private val pendingDialogs = java.util.concurrent.ConcurrentHashMap.newKeySet<CompletableFuture<Boolean>>()

    /** À appeler dans `onDestroy` : une boîte attendue par Ocara ne bloque pas son thread. */
    fun release() {
        pendingDialogs.forEach { it.complete(false) }
        pendingDialogs.clear()
    }

    /** Retour intercepté : gardé ici, une Activity recréée le reprend. */
    @Volatile
    private var interceptBack = false

    /** À appeler dans `onCreate` (après [OcaraBridge.attach]). */
    fun install(activity: ComponentActivity) {
        val callback = object : OnBackPressedCallback(interceptBack) {
            override fun handleOnBackPressed() {
                OcaraBridge.nativeEvent("back", "null")
            }
        }
        activity.onBackPressedDispatcher.addCallback(activity, callback)
        backCallback = callback
    }

    private fun activity(): ComponentActivity =
        OcaraBridge.activity as? ComponentActivity
            ?: throw OcaraAndroidError(OcaraAndroidError.UNAVAILABLE, "aucune Activity au premier plan (OcaraBridge.attach)")

    /** Exécute [action] sur le thread principal et attend son résultat. */
    private fun <T> onUiThread(action: (ComponentActivity) -> T): T {
        val activity = activity()
        val result = CompletableFuture<T>()
        activity.runOnUiThread {
            try {
                result.complete(action(activity))
            } catch (e: Throwable) {
                result.completeExceptionally(e)
            }
        }
        return try {
            result.get()
        } catch (e: java.util.concurrent.ExecutionException) {
            throw e.cause ?: e
        }
    }

    /** Le bouton retour remonte à Ocara (`AndroidUI::listen("back", …)`) au lieu de fermer l'app. */
    @JvmStatic
    fun setInterceptBack(enabled: Long) = onUiThread {
        interceptBack = enabled != 0L
        backCallback?.isEnabled = interceptBack
    }

    @JvmStatic
    fun setSystemBarsStyle(style: String) = onUiThread { activity ->
        val light = when (style) {
            "light" -> true
            "dark" -> false
            else -> throw OcaraAndroidError(OcaraAndroidError.ARGUMENT, "style : 'light' ou 'dark' attendu, reçu '$style'")
        }
        val controller = WindowCompat.getInsetsController(activity.window, activity.window.decorView)
        controller.isAppearanceLightStatusBars = light
        controller.isAppearanceLightNavigationBars = light
    }

    @JvmStatic
    fun setFullscreen(enabled: Long) = onUiThread { activity ->
        val controller = WindowCompat.getInsetsController(activity.window, activity.window.decorView)
        if (enabled != 0L) {
            controller.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            controller.hide(WindowInsetsCompat.Type.systemBars())
        } else {
            controller.show(WindowInsetsCompat.Type.systemBars())
        }
    }

    @JvmStatic
    fun setOrientation(orientation: String) = onUiThread { activity ->
        activity.requestedOrientation = when (orientation) {
            "portrait" -> ActivityInfo.SCREEN_ORIENTATION_USER_PORTRAIT
            "landscape" -> ActivityInfo.SCREEN_ORIENTATION_USER_LANDSCAPE
            "auto" -> ActivityInfo.SCREEN_ORIENTATION_FULL_USER
            else -> throw OcaraAndroidError(OcaraAndroidError.ARGUMENT, "orientation : 'portrait', 'landscape' ou 'auto' attendu, reçu '$orientation'")
        }
    }

    @JvmStatic
    fun setKeepScreenOn(enabled: Long) = onUiThread { activity ->
        if (enabled != 0L) {
            activity.window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        } else {
            activity.window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        }
    }

    @JvmStatic
    fun vibrate(ms: Long) {
        if (ms <= 0) throw OcaraAndroidError(OcaraAndroidError.ARGUMENT, "durée de vibration invalide : $ms ms")
        val context = OcaraBridge.context
        val vibrator = if (Build.VERSION.SDK_INT >= 31) {
            context.getSystemService(VibratorManager::class.java).defaultVibrator
        } else {
            @Suppress("DEPRECATION")
            context.getSystemService(Context.VIBRATOR_SERVICE) as Vibrator
        }
        if (!vibrator.hasVibrator()) throw OcaraAndroidError(OcaraAndroidError.DEVICE, "l'appareil n'a pas de vibreur")
        if (Build.VERSION.SDK_INT >= 26) {
            vibrator.vibrate(VibrationEffect.createOneShot(ms, VibrationEffect.DEFAULT_AMPLITUDE))
        } else {
            @Suppress("DEPRECATION")
            vibrator.vibrate(ms)
        }
    }

    @JvmStatic
    fun share(text: String, title: String) = onUiThread { activity ->
        val send = Intent(Intent.ACTION_SEND).apply {
            type = "text/plain"
            putExtra(Intent.EXTRA_TEXT, text)
        }
        activity.startActivity(Intent.createChooser(send, title.ifEmpty { null }))
    }

    @JvmStatic
    fun alert(title: String, text: String) {
        dialog(title, text, cancellable = false)
    }

    @JvmStatic
    fun confirm(title: String, text: String): Boolean = dialog(title, text, cancellable = true)

    /** Boîte bloquante (thread Ocara) : vrai si l'utilisateur valide. */
    private fun dialog(title: String, text: String, cancellable: Boolean): Boolean {
        requireBackgroundThread(if (cancellable) "AndroidUI::confirm" else "AndroidUI::alert")
        val answer = CompletableFuture<Boolean>()
        pendingDialogs.add(answer)
        val activity = activity()
        activity.runOnUiThread {
            val builder = AlertDialog.Builder(activity)
                .setTitle(title)
                .setMessage(text)
                .setPositiveButton(android.R.string.ok) { _, _ -> answer.complete(true) }
                .setOnDismissListener { answer.complete(false) }
            if (cancellable) builder.setNegativeButton(android.R.string.cancel) { _, _ -> answer.complete(false) }
            builder.show()
        }
        return try { answer.get() } finally { pendingDialogs.remove(answer) }
    }

    @JvmStatic
    fun toast(text: String) = onUiThread { activity -> Toast.makeText(activity, text, Toast.LENGTH_SHORT).show() }

    @JvmStatic
    fun close() = onUiThread { activity -> activity.finish() }
}
