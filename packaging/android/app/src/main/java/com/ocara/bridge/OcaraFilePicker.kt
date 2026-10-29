package com.ocara.bridge

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import android.webkit.MimeTypeMap
import java.io.File
import java.util.concurrent.CompletableFuture

/**
 * `ui.pickFile(extensions)` d'ocara.UI : sélecteur de documents du système
 * (`ACTION_OPEN_DOCUMENT`). Le document choisi est copié dans le cache de
 * l'app : Ocara reçoit un vrai chemin, comme sur desktop ; `""` si annulé.
 */
object OcaraFilePicker {
    private const val REQUEST = 0x0CA1
    @Volatile
    private var pending: CompletableFuture<Uri?>? = null

    /** `extensions` : `"png,jpg"` (vide : tous les fichiers). Thread Ocara. */
    @JvmStatic
    fun pickFile(extensions: String): String {
        requireBackgroundThread("UI.pickFile")
        val activity = OcaraBridge.activity
            ?: throw OcaraAndroidError(OcaraAndroidError.UNAVAILABLE, "aucune Activity au premier plan")
        val types = extensions.split(",").map { it.trim().trimStart('.') }.filter { it.isNotEmpty() }
            .mapNotNull { MimeTypeMap.getSingleton().getMimeTypeFromExtension(it.lowercase()) }
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("*/*")
        if (types.isNotEmpty()) intent.putExtra(Intent.EXTRA_MIME_TYPES, types.toTypedArray())
        val answer = CompletableFuture<Uri?>()
        pending = answer
        activity.runOnUiThread {
            @Suppress("DEPRECATION")
            activity.startActivityForResult(intent, REQUEST)
        }
        val uri = answer.get() ?: return ""
        return copyToCache(uri).absolutePath
    }

    /** À appeler depuis `Activity.onActivityResult`. */
    fun onResult(requestCode: Int, resultCode: Int, data: Intent?) {
        if (requestCode != REQUEST) return
        pending?.complete(if (resultCode == Activity.RESULT_OK) data?.data else null)
        pending = null
    }

    /** Activity détruite : un choix attendu rend « annulé ». */
    fun release() {
        pending?.complete(null)
        pending = null
    }

    private fun copyToCache(uri: Uri): File {
        val resolver = OcaraBridge.context.contentResolver
        val name = resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
            if (it.moveToFirst()) it.getString(0) else null
        } ?: "document"
        val dir = File(OcaraBridge.context.cacheDir, "picked").apply { mkdirs() }
        val file = File(dir, File(name).name)
        resolver.openInputStream(uri)?.use { input -> file.outputStream().use { input.copyTo(it) } }
            ?: throw OcaraAndroidError(OcaraAndroidError.DEVICE, "lecture du document choisi impossible")
        return file
    }
}
