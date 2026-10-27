package com.ocara.bridge

import android.content.ContentValues
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.webkit.MimeTypeMap
import java.io.File
import java.io.IOException
import java.io.InputStream

/** `ocara.AndroidFiles` : répertoires de l'app, assets de l'APK, Téléchargements. */
object OcaraFiles {
    @JvmStatic
    fun filesDir(): String = OcaraBridge.context.filesDir.absolutePath

    @JvmStatic
    fun cacheDir(): String = OcaraBridge.context.cacheDir.absolutePath

    @JvmStatic
    fun externalFilesDir(): String =
        OcaraBridge.context.getExternalFilesDir(null)?.absolutePath
            ?: throw OcaraAndroidError(OcaraAndroidError.DEVICE, "stockage externe de l'app indisponible")

    @JvmStatic
    fun listAssets(dir: String): Array<String> = OcaraBridge.context.assets.list(dir.trim('/')) ?: emptyArray()

    @JvmStatic
    fun readAsset(path: String): String = openAsset(path).use { it.readBytes().toString(Charsets.UTF_8) }

    @JvmStatic
    fun copyAsset(path: String, dest: String) {
        val target = OcaraBridge.file(dest)
        target.parentFile?.mkdirs()
        openAsset(path).use { input -> target.outputStream().use { input.copyTo(it) } }
    }

    /** Copie [source] dans le dossier Téléchargements partagé ; rend l'URI (Android ≥ 10) ou le chemin. */
    @JvmStatic
    fun saveToDownloads(source: String, name: String): String {
        val file = OcaraBridge.file(source)
        if (!file.isFile) throw OcaraAndroidError(OcaraAndroidError.IO, "fichier introuvable : '$source'")
        if (Build.VERSION.SDK_INT >= 29) return insertDownload(file, name)
        if (!OcaraPermissions.isGranted("android.permission.WRITE_EXTERNAL_STORAGE")) {
            throw OcaraAndroidError(OcaraAndroidError.PERMISSION, "permission WRITE_EXTERNAL_STORAGE non accordée (Android < 10)")
        }
        @Suppress("DEPRECATION")
        val dir = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS)
        dir.mkdirs()
        return file.copyTo(File(dir, name), overwrite = true).absolutePath
    }

    private fun insertDownload(file: File, name: String): String {
        val resolver = OcaraBridge.context.contentResolver
        val mime = MimeTypeMap.getSingleton().getMimeTypeFromExtension(file.extension.lowercase()) ?: "application/octet-stream"
        val values = ContentValues().apply {
            put(MediaStore.Downloads.DISPLAY_NAME, name)
            put(MediaStore.Downloads.MIME_TYPE, mime)
            put(MediaStore.Downloads.IS_PENDING, 1)
        }
        val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
            ?: throw OcaraAndroidError(OcaraAndroidError.IO, "Téléchargements : entrée impossible à créer pour '$name'")
        resolver.openOutputStream(uri)?.use { out -> file.inputStream().use { it.copyTo(out) } }
            ?: throw OcaraAndroidError(OcaraAndroidError.IO, "Téléchargements : écriture impossible de '$name'")
        values.clear()
        values.put(MediaStore.Downloads.IS_PENDING, 0)
        resolver.update(uri, values, null, null)
        return uri.toString()
    }

    private fun openAsset(path: String): InputStream =
        try {
            OcaraBridge.context.assets.open(path.trimStart('/'))
        } catch (_: IOException) {
            throw OcaraAndroidError(OcaraAndroidError.IO, "asset introuvable : '$path'")
        }
}
