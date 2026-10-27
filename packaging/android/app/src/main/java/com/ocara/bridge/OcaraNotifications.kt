package com.ocara.bridge

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.os.Build
import java.util.concurrent.atomic.AtomicInteger

/** `ocara.AndroidNotifications` : un canal « ocara » créé au premier envoi. */
object OcaraNotifications {
    private const val CHANNEL = "ocara"
    private val nextId = AtomicInteger(0)

    private fun manager(): NotificationManager = OcaraBridge.context.getSystemService(NotificationManager::class.java)

    @JvmStatic
    fun show(title: String, text: String): Long {
        if (!OcaraPermissions.isGranted("notifications")) {
            throw OcaraAndroidError(OcaraAndroidError.PERMISSION, "permission POST_NOTIFICATIONS non accordée (AndroidPermissions::request(\"notifications\"))")
        }
        val context = OcaraBridge.context
        val builder = if (Build.VERSION.SDK_INT >= 26) {
            ensureChannel()
            Notification.Builder(context, CHANNEL)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(context)
        }
        builder.setSmallIcon(context.applicationInfo.icon)
            .setContentTitle(title)
            .setContentText(text)
            .setAutoCancel(true)
        context.packageManager.getLaunchIntentForPackage(context.packageName)?.let {
            builder.setContentIntent(PendingIntent.getActivity(context, 0, it, PendingIntent.FLAG_IMMUTABLE))
        }
        val id = nextId.incrementAndGet()
        manager().notify(id, builder.build())
        return id.toLong()
    }

    @JvmStatic
    fun cancel(id: Long) = manager().cancel(id.toInt())

    @JvmStatic
    fun areEnabled(): Boolean = manager().areNotificationsEnabled()

    private fun ensureChannel() {
        if (Build.VERSION.SDK_INT < 26 || manager().getNotificationChannel(CHANNEL) != null) return
        val context = OcaraBridge.context
        val name = context.applicationInfo.loadLabel(context.packageManager).toString()
        manager().createNotificationChannel(NotificationChannel(CHANNEL, name, NotificationManager.IMPORTANCE_DEFAULT))
    }
}
