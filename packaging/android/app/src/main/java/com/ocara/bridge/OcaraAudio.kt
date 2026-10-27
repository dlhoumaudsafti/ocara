package com.ocara.bridge

import android.media.MediaPlayer

/** `ocara.AndroidAudio` : lecture d'un fichier audio par `MediaPlayer`, une à la fois. */
object OcaraAudio {
    private var player: MediaPlayer? = null
    private var volume = 1f

    @JvmStatic
    @Synchronized
    fun play(path: String) {
        release()
        val file = OcaraBridge.file(path)
        if (!file.isFile) throw OcaraAndroidError(OcaraAndroidError.IO, "fichier audio introuvable : '$path'")
        val created = MediaPlayer()
        try {
            created.setDataSource(file.absolutePath)
            created.setVolume(volume, volume)
            created.prepare()
            created.start()
        } catch (e: Exception) {
            created.release()
            throw OcaraAndroidError(OcaraAndroidError.IO, "lecture impossible de '$path' : ${e.message}")
        }
        player = created
    }

    @JvmStatic
    @Synchronized
    fun pause() = current().pause()

    @JvmStatic
    @Synchronized
    fun resume() = current().start()

    @JvmStatic
    @Synchronized
    fun stop() = release()

    @JvmStatic
    @Synchronized
    fun isPlaying(): Boolean = player?.isPlaying == true

    @JvmStatic
    @Synchronized
    fun setVolume(value: Double) {
        if (value !in 0.0..1.0) throw OcaraAndroidError(OcaraAndroidError.ARGUMENT, "volume hors de 0.0..1.0 : $value")
        volume = value.toFloat()
        player?.setVolume(volume, volume)
    }

    private fun current(): MediaPlayer = player ?: throw OcaraAndroidError(OcaraAndroidError.STATE, "aucune lecture en cours (AndroidAudio::play)")

    private fun release() {
        player?.release()
        player = null
    }
}
