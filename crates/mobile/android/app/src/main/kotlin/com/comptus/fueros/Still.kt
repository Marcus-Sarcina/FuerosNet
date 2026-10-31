package com.comptus.fueros

import android.content.Context
import android.graphics.ImageFormat
import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraDevice
import android.hardware.camera2.CameraManager
import android.hardware.camera2.CaptureRequest
import android.media.ImageReader
import android.os.Handler
import android.os.HandlerThread
import android.view.Surface
import java.util.concurrent.atomic.AtomicBoolean
import timber.log.Timber

/**
 * **One photograph, taken by this shell** (`light-client-requirements.md`
 * §8.1's `capture` and `scan`).
 *
 * **Why the shell and not the platform's camera app.** The capture is a
 * device capability under the person's consent, and handing the act to
 * another application would put a third party between the person and
 * what this device then claims it took. The same reason puts the
 * ceremony's camera here ([QrCamera]): the shell keeps the sensor, the
 * kernel keeps the policy, and what crosses out of here is bytes.
 *
 * **It is [QrCamera]'s shape with the decoding taken out**: one open per
 * capture, a preview surface the caller supplies, the camera given back
 * whatever happens, and every refusal said in words rather than thrown
 * where nothing catches it. What it does not carry is the watchdog, the
 * metering steering and the frame tally — those earned their place
 * against a symbol read across a room, and a photograph the person
 * frames and takes needs none of them.
 */
class Still(private val context: Context) {

    private var manager: CameraManager? = null
    private var id: String? = null
    private var device: CameraDevice? = null
    private var session: CameraCaptureSession? = null
    private var reader: ImageReader? = null
    private var thread: HandlerThread? = null
    private var handler: Handler? = null
    private val open = AtomicBoolean(false)
    private var sensorOrientation = 0
    private var failed: ((String) -> Unit)? = null
    private var got: ((ByteArray) -> Unit)? = null

    /** The largest JPEG this takes: a photograph of a document is legible
     *  well under it, and every byte is one the person waits on. */
    private val maxPixels = 8_000_000

    /**
     * Open the rear camera and run the preview on `preview`.  The refusal
     * in words where it will not open, and `null` where it did; `failed`
     * carries a refusal that happens later, on the camera's own thread.
     */
    fun start(preview: Surface?, failed: (String) -> Unit): String? {
        if (open.get()) return null
        this.failed = failed
        val manager = context.getSystemService(Context.CAMERA_SERVICE) as? CameraManager
            ?: return refused("this device exposes no camera service")
        val id = rear(manager) ?: return refused("this device has no rear camera")
        val characteristics = manager.getCameraCharacteristics(id)
        sensorOrientation = characteristics.get(CameraCharacteristics.SENSOR_ORIENTATION) ?: 0
        val size = characteristics
            .get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP)
            ?.getOutputSizes(ImageFormat.JPEG)
            ?.filter { it.width * it.height <= maxPixels }
            ?.maxByOrNull { it.width * it.height }
            ?: return refused("this camera offers no still size the app can use")
        this.manager = manager
        this.id = id
        val t = HandlerThread("still").also { it.start() }
        thread = t
        val h = Handler(t.looper)
        handler = h
        val r = ImageReader.newInstance(size.width, size.height, ImageFormat.JPEG, 2)
        reader = r
        r.setOnImageAvailableListener({ ir ->
            val image = ir.acquireNextImage() ?: return@setOnImageAvailableListener
            try {
                val buf = image.planes[0].buffer
                val bytes = ByteArray(buf.remaining())
                buf.get(bytes)
                Diag.event("camera", "which" to "still", "op" to "taken", "bytes" to bytes.size)
                val deliver = got
                got = null
                deliver?.invoke(bytes)
            } catch (e: Exception) {
                onThread("frame", e)
            } finally {
                image.close()
            }
        }, h)
        open.set(true)
        return try {
            openCamera(manager, id, r, preview, h)
            null
        } catch (e: SecurityException) {
            close()
            refused("the camera permission is not held")
        } catch (e: Exception) {
            close()
            refused("the camera would not open: $e")
        }
    }

    /**
     * Take one photograph.  `rotationDegrees` is the display's own
     * rotation, so the JPEG carries the orientation the person saw rather
     * than the sensor's.
     */
    fun take(rotationDegrees: Int, got: (ByteArray) -> Unit): String? {
        val camera = device ?: return "the camera is not open yet"
        val s = session ?: return "the camera is not ready yet"
        val r = reader ?: return "the camera is not open yet"
        this.got = got
        return try {
            val b = camera.createCaptureRequest(CameraDevice.TEMPLATE_STILL_CAPTURE)
            b.addTarget(r.surface)
            b.set(CaptureRequest.CONTROL_AF_MODE, CaptureRequest.CONTROL_AF_MODE_CONTINUOUS_PICTURE)
            b.set(CaptureRequest.CONTROL_AE_MODE, CaptureRequest.CONTROL_AE_MODE_ON)
            b.set(CaptureRequest.JPEG_QUALITY, 92.toByte())
            // the rear camera's formula: the sensor's own orientation less
            // the device's, so the image stands the way the person held it
            b.set(
                CaptureRequest.JPEG_ORIENTATION,
                ((sensorOrientation - rotationDegrees) + 360) % 360,
            )
            s.capture(b.build(), null, handler)
            null
        } catch (e: Exception) {
            onThread("capture", e)
            "the camera would not take it: $e"
        }
    }

    /** Give the camera back.  Safe at any point, and more than once. */
    fun close() {
        open.set(false)
        got = null
        runCatching { session?.close() }
        session = null
        runCatching { device?.close() }
        device = null
        runCatching { reader?.close() }
        reader = null
        runCatching { thread?.quitSafely() }
        thread = null
        handler = null
    }

    private fun rear(manager: CameraManager): String? =
        manager.cameraIdList.firstOrNull { id ->
            manager.getCameraCharacteristics(id).get(CameraCharacteristics.LENS_FACING) ==
                CameraCharacteristics.LENS_FACING_BACK
        }

    private fun refused(why: String): String {
        Diag.warn("camera", "which" to "still", "op" to "open", "error" to why)
        return why
    }

    /** A failure on the camera's own thread: said once, the camera given
     *  back, and never a crash outside any try the caller holds. */
    private fun onThread(op: String, e: Throwable) {
        Timber.w(e, "still camera %s", op)
        Diag.warn("camera", "which" to "still", "op" to op, "error" to e.toString())
        val tell = failed
        failed = null
        close()
        tell?.invoke("the camera stopped: $e")
    }

    private fun openCamera(
        manager: CameraManager,
        id: String,
        reader: ImageReader,
        preview: Surface?,
        h: Handler,
    ) {
        manager.openCamera(
            id,
            object : CameraDevice.StateCallback() {
                override fun onOpened(camera: CameraDevice) {
                    device = camera
                    Diag.event("camera", "which" to "still", "op" to "open")
                    val surfaces = listOfNotNull(reader.surface, preview)
                    try {
                        @Suppress("DEPRECATION")
                        camera.createCaptureSession(
                            surfaces,
                            object : CameraCaptureSession.StateCallback() {
                                override fun onConfigured(s: CameraCaptureSession) {
                                    if (!open.get()) {
                                        runCatching { s.close() }
                                        return
                                    }
                                    session = s
                                    // the preview alone repeats; the still
                                    // is one request at the person's press
                                    if (preview == null) return
                                    try {
                                        val b = camera.createCaptureRequest(
                                            CameraDevice.TEMPLATE_PREVIEW,
                                        )
                                        b.addTarget(preview)
                                        b.set(
                                            CaptureRequest.CONTROL_AF_MODE,
                                            CaptureRequest.CONTROL_AF_MODE_CONTINUOUS_PICTURE,
                                        )
                                        s.setRepeatingRequest(b.build(), null, h)
                                    } catch (e: Exception) {
                                        onThread("preview", e)
                                    }
                                }

                                override fun onConfigureFailed(s: CameraCaptureSession) {
                                    onThread("configure", IllegalStateException("session refused"))
                                }
                            },
                            h,
                        )
                    } catch (e: Exception) {
                        onThread("session", e)
                    }
                }

                override fun onDisconnected(camera: CameraDevice) {
                    onThread("disconnected", IllegalStateException("the camera was taken away"))
                }

                override fun onError(camera: CameraDevice, error: Int) {
                    onThread("error", IllegalStateException("camera error $error"))
                }
            },
            h,
        )
    }
}
