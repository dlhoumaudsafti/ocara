package com.ocara.bridge

import android.content.Context
import android.graphics.ImageFormat
import android.graphics.SurfaceTexture
import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraDevice
import android.hardware.camera2.CameraManager
import android.hardware.camera2.CaptureRequest
import android.hardware.camera2.CaptureResult
import android.hardware.camera2.TotalCaptureResult
import android.media.ImageReader
import android.os.Handler
import android.os.HandlerThread
import android.view.Surface
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ExecutionException
import java.util.concurrent.TimeUnit
import java.util.concurrent.TimeoutException
import java.util.concurrent.atomic.AtomicReference

/**
 * `ocara.AndroidCamera` : une photo JPEG sans aperçu affiché. Un aperçu
 * vers une surface invisible tourne jusqu'à ce que l'exposition automatique
 * ait convergé (au plus [MAX_WARM_UP_MS]) avant la prise.
 */
object OcaraCamera {
    private const val MIN_WARM_UP_MS = 300L
    private const val MAX_WARM_UP_MS = 3000L
    private const val TIMEOUT_S = 10L

    private fun manager(): CameraManager = OcaraBridge.context.getSystemService(Context.CAMERA_SERVICE) as CameraManager

    @JvmStatic
    fun count(): Long = manager().cameraIdList.size.toLong()

    @JvmStatic
    fun takePhoto(path: String, facing: String) {
        val lens = when (facing) {
            "", "back" -> CameraCharacteristics.LENS_FACING_BACK
            "front" -> CameraCharacteristics.LENS_FACING_FRONT
            else -> throw OcaraAndroidError(OcaraAndroidError.ARGUMENT, "facing : 'back' ou 'front' attendu, reçu '$facing'")
        }
        if (!OcaraPermissions.isGranted("camera")) {
            throw OcaraAndroidError(OcaraAndroidError.PERMISSION, "permission CAMERA non accordée (AndroidPermissions::request(\"camera\"))")
        }
        requireBackgroundThread("AndroidCamera::takePhoto")
        val manager = manager()
        val id = manager.cameraIdList.firstOrNull { manager.getCameraCharacteristics(it).get(CameraCharacteristics.LENS_FACING) == lens }
            ?: throw OcaraAndroidError(OcaraAndroidError.DEVICE, "aucune caméra '${facing.ifEmpty { "back" }}' sur l'appareil")
        val bytes = capture(manager, id)
        val target = OcaraBridge.file(path)
        target.parentFile?.mkdirs()
        target.writeBytes(bytes)
    }

    private fun capture(manager: CameraManager, id: String): ByteArray {
        val characteristics = manager.getCameraCharacteristics(id)
        val size = characteristics.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP)!!
            .getOutputSizes(ImageFormat.JPEG).maxBy { it.width.toLong() * it.height }
        val thread = HandlerThread("OcaraCamera").apply { start() }
        val handler = Handler(thread.looper)
        val reader = ImageReader.newInstance(size.width, size.height, ImageFormat.JPEG, 1)
        val texture = SurfaceTexture(0).apply { setDefaultBufferSize(640, 480) }
        val preview = Surface(texture)
        val device = AtomicReference<CameraDevice?>()
        val result = CompletableFuture<ByteArray>()
        try {
            reader.setOnImageAvailableListener({ r ->
                r.acquireLatestImage()?.use { image ->
                    val buffer = image.planes[0].buffer
                    result.complete(ByteArray(buffer.remaining()).also { buffer.get(it) })
                }
            }, handler)
            manager.openCamera(id, object : CameraDevice.StateCallback() {
                override fun onOpened(camera: CameraDevice) {
                    device.set(camera)
                    startSession(camera, preview, reader, characteristics, handler, result)
                }

                override fun onDisconnected(camera: CameraDevice) {
                    result.completeExceptionally(OcaraAndroidError(OcaraAndroidError.DEVICE, "caméra déconnectée"))
                }

                override fun onError(camera: CameraDevice, error: Int) {
                    result.completeExceptionally(OcaraAndroidError(OcaraAndroidError.DEVICE, "erreur de la caméra (code $error)"))
                }
            }, handler)
            return try {
                result.get(TIMEOUT_S, TimeUnit.SECONDS)
            } catch (_: TimeoutException) {
                throw OcaraAndroidError(OcaraAndroidError.TIMEOUT, "aucune photo en $TIMEOUT_S s")
            } catch (e: ExecutionException) {
                throw e.cause ?: e
            }
        } finally {
            device.get()?.close()
            reader.close()
            preview.release()
            texture.release()
            thread.quitSafely()
        }
    }

    /** Exposition automatique stable (ou sans objet sur cet appareil). */
    private fun exposed(frame: TotalCaptureResult): Boolean =
        when (frame.get(CaptureResult.CONTROL_AE_STATE)) {
            null, CaptureResult.CONTROL_AE_STATE_CONVERGED,
            CaptureResult.CONTROL_AE_STATE_FLASH_REQUIRED, CaptureResult.CONTROL_AE_STATE_LOCKED -> true
            else -> false
        }

    private fun startSession(
        camera: CameraDevice,
        preview: Surface,
        reader: ImageReader,
        characteristics: CameraCharacteristics,
        handler: Handler,
        result: CompletableFuture<ByteArray>,
    ) {
        @Suppress("DEPRECATION")
        camera.createCaptureSession(listOf(preview, reader.surface), object : CameraCaptureSession.StateCallback() {
            override fun onConfigured(session: CameraCaptureSession) {
                val started = System.currentTimeMillis()
                var shot = false
                val shoot = {
                    if (!shot && !result.isDone) {
                        shot = true
                        val still = camera.createCaptureRequest(CameraDevice.TEMPLATE_STILL_CAPTURE).apply {
                            addTarget(reader.surface)
                            set(CaptureRequest.JPEG_ORIENTATION, characteristics.get(CameraCharacteristics.SENSOR_ORIENTATION) ?: 0)
                        }.build()
                        session.capture(still, null, handler)
                    }
                }
                val warmUp = camera.createCaptureRequest(CameraDevice.TEMPLATE_PREVIEW).apply { addTarget(preview) }.build()
                session.setRepeatingRequest(warmUp, object : CameraCaptureSession.CaptureCallback() {
                    override fun onCaptureCompleted(session: CameraCaptureSession, request: CaptureRequest, frame: TotalCaptureResult) {
                        val elapsed = System.currentTimeMillis() - started
                        if (elapsed >= MIN_WARM_UP_MS && (exposed(frame) || elapsed >= MAX_WARM_UP_MS)) shoot()
                    }
                }, handler)
            }

            override fun onConfigureFailed(session: CameraCaptureSession) {
                result.completeExceptionally(OcaraAndroidError(OcaraAndroidError.DEVICE, "configuration de la capture impossible"))
            }
        }, handler)
    }
}
