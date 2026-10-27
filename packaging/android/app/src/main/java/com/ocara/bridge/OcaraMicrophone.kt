package com.ocara.bridge

import android.media.MediaRecorder
import android.os.Build

/** `ocara.AndroidMicrophone` : enregistrement AAC (`.m4a`) par `MediaRecorder`. */
object OcaraMicrophone {
    private var recorder: MediaRecorder? = null

    @JvmStatic
    @Synchronized
    fun start(path: String) {
        if (recorder != null) throw OcaraAndroidError(OcaraAndroidError.STATE, "un enregistrement est déjà en cours")
        if (!OcaraPermissions.isGranted("microphone")) {
            throw OcaraAndroidError(OcaraAndroidError.PERMISSION, "permission RECORD_AUDIO non accordée (AndroidPermissions::request(\"microphone\"))")
        }
        val target = OcaraBridge.file(path)
        target.parentFile?.mkdirs()
        val created = if (Build.VERSION.SDK_INT >= 31) {
            MediaRecorder(OcaraBridge.context)
        } else {
            @Suppress("DEPRECATION")
            MediaRecorder()
        }
        try {
            created.setAudioSource(MediaRecorder.AudioSource.MIC)
            created.setOutputFormat(MediaRecorder.OutputFormat.MPEG_4)
            created.setAudioEncoder(MediaRecorder.AudioEncoder.AAC)
            created.setAudioSamplingRate(44100)
            created.setAudioEncodingBitRate(128000)
            created.setOutputFile(target.absolutePath)
            created.prepare()
            created.start()
        } catch (e: Exception) {
            created.release()
            throw OcaraAndroidError(OcaraAndroidError.DEVICE, "micro indisponible : ${e.message}")
        }
        recorder = created
    }

    @JvmStatic
    @Synchronized
    fun stop() {
        val current = recorder ?: throw OcaraAndroidError(OcaraAndroidError.STATE, "aucun enregistrement en cours")
        recorder = null
        try {
            current.stop()
        } catch (_: RuntimeException) {
            throw OcaraAndroidError(OcaraAndroidError.IO, "enregistrement vide ou invalide (arrêté trop tôt ?)")
        } finally {
            current.release()
        }
    }

    @JvmStatic
    @Synchronized
    fun isRecording(): Boolean = recorder != null
}
