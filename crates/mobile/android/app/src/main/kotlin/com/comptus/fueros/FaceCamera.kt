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
import android.util.Log
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/**
 * The guided capture's camera (D4): the counterparty's face, one frame per
 * prompt, on the SELFIE camera because the phone is already turned to face
 * them from D2.
 *
 * **This is the kernel's `Camera` seam, and the seam is synchronous.**
 * `rhtn-client` calls `capture(ask)` once per prompt on its own thread and
 * blocks for the pixels; this holds the camera open across the sequence and
 * answers each call with the next frame. The sequence — how many frames,
 * the prompts, the spacing — is the kernel's (design §7.5), so this returns
 * a frame when asked and decides none of it.
 *
 * **What crosses inward is pixels and nothing else**
 * (`light-client-requirements.md` §1.3). The kernel strips a frame to its
 * pixels regardless; this hands over exactly the luminance plane and never
 * the location, orientation or timestamp the platform attached.
 *
 * **The disclosures were read at D1.5 and the device faces away**, so this
 * shows the person nothing and asks nothing: the prompt is for the
 * counterparty, spoken or toned by the screen above, not a dialog here.
 *
 * **NOT RUN ON HARDWARE**, like every camera and radio half in this shell.
 * The faces that defeat a liveness check and the frames a real sensor
 * delivers have never reached it.
 */
class FaceCamera(private val context: Context) {

    private val width = 640
    private val height = 480

    private var thread: HandlerThread? = null
    private var handler: Handler? = null
    private var device: CameraDevice? = null
    private var session: CameraCaptureSession? = null
    private var reader: ImageReader? = null
    private val latest = AtomicReference<ByteArray?>(null)

    /** Open the selfie camera and begin delivering frames. Idempotent. */
    @Synchronized
    fun open(): String? {
        if (device != null) return null
        val manager = context.getSystemService(Context.CAMERA_SERVICE) as? CameraManager
            ?: return "this device exposes no camera service"
        val id = manager.cameraIdList.firstOrNull {
            manager.getCameraCharacteristics(it)
                .get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_FRONT
        } ?: return "no selfie camera"
        val t = HandlerThread("face").also { it.start() }
        thread = t
        val h = Handler(t.looper)
        handler = h
        val r = ImageReader.newInstance(width, height, ImageFormat.YUV_420_888, 3)
        reader = r
        r.setOnImageAvailableListener({ ir ->
            val image = ir.acquireLatestImage() ?: return@setOnImageAvailableListener
            try {
                // the Y plane alone: pixels, and nothing the pipeline
                // attached. A template is derived from luminance; colour
                // and the metadata are left on the platform's side.
                val y = image.planes[0].buffer
                val bytes = ByteArray(y.remaining())
                y.get(bytes)
                latest.set(bytes)
            } finally {
                image.close()
            }
        }, h)
        val opened = CountDownLatch(1)
        val why = AtomicReference<String?>("the camera did not open")
        return try {
            manager.openCamera(
                id,
                object : CameraDevice.StateCallback() {
                    override fun onOpened(camera: CameraDevice) {
                        device = camera
                        try {
                            @Suppress("DEPRECATION")
                            camera.createCaptureSession(
                                listOf(r.surface),
                                object : CameraCaptureSession.StateCallback() {
                                    override fun onConfigured(s: CameraCaptureSession) {
                                        session = s
                                        val b = camera.createCaptureRequest(
                                            CameraDevice.TEMPLATE_PREVIEW,
                                        )
                                        b.addTarget(r.surface)
                                        b.set(
                                            CaptureRequest.CONTROL_AF_MODE,
                                            CaptureRequest.CONTROL_AF_MODE_CONTINUOUS_PICTURE,
                                        )
                                        s.setRepeatingRequest(b.build(), null, h)
                                        why.set(null)
                                        opened.countDown()
                                    }

                                    override fun onConfigureFailed(s: CameraCaptureSession) {
                                        why.set("the capture session would not configure")
                                        opened.countDown()
                                    }
                                },
                                h,
                            )
                        } catch (e: Exception) {
                            why.set("the capture session failed: $e")
                            opened.countDown()
                        }
                    }

                    override fun onDisconnected(camera: CameraDevice) {
                        why.set("the camera disconnected")
                        opened.countDown()
                        close()
                    }

                    override fun onError(camera: CameraDevice, error: Int) {
                        why.set("the camera errored: $error")
                        opened.countDown()
                        close()
                    }
                },
                h,
            )
            opened.await(5, TimeUnit.SECONDS)
            why.get()
        } catch (e: SecurityException) {
            close()
            "the camera permission is not held"
        } catch (e: Exception) {
            close()
            "the camera would not open: $e"
        }
    }

    /**
     * One frame for the kernel's prompt: the most recent the sensor has
     * delivered, waiting briefly for the first. Empty where none came —
     * the kernel checks the template length and an empty frame fails it,
     * which is a capture that did not happen rather than one that lied.
     */
    fun frame(): ByteArray {
        val deadline = System.currentTimeMillis() + 2_000
        while (latest.get() == null && System.currentTimeMillis() < deadline) {
            Thread.sleep(50)
        }
        return latest.get() ?: ByteArray(0)
    }

    @Synchronized
    fun close() {
        runCatching { session?.close() }
        runCatching { device?.close() }
        runCatching { reader?.close() }
        runCatching { thread?.quitSafely() }
        session = null
        device = null
        reader = null
        thread = null
        handler = null
        latest.set(null)
    }
}
