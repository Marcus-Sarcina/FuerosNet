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
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
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

    /** For the `face.frames` event: frames the sensor delivered, frames
     *  handed to the kernel, waits that ran out, and the frame's size.
     *  Never a pixel. */
    private val delivered = AtomicInteger(0)
    private val served = AtomicInteger(0)
    private val frameTimeouts = AtomicInteger(0)
    private var openTimedOut = false
    @Volatile private var frameBytes = 0

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
                delivered.incrementAndGet()
                frameBytes = bytes.size
            } finally {
                image.close()
            }
        }, h)
        val opened = CountDownLatch(1)
        val why = AtomicReference<String?>("the camera did not open")
        val startedMs = Diag.ms()
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
                                        Diag.event(
                                            "camera",
                                            "which" to "face",
                                            "op" to "request",
                                            "template" to "preview",
                                            "af" to "continuous_picture",
                                            "exposure" to "auto",
                                            "antibanding" to "default",
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
                        Diag.warn("camera", "which" to "face", "op" to "disconnected")
                        why.set("the camera disconnected")
                        opened.countDown()
                        close()
                    }

                    override fun onError(camera: CameraDevice, error: Int) {
                        Diag.warn("camera", "which" to "face", "op" to "error", "error" to error)
                        why.set("the camera errored: $error")
                        opened.countDown()
                        close()
                    }
                },
                h,
            )
            // the 5 s is the open timeout the plan's `face.frames` row counts
            openTimedOut = !opened.await(5, TimeUnit.SECONDS)
            val out = if (openTimedOut) "the camera did not open in time" else why.get()
            if (out == null) {
                Diag.event(
                    "camera",
                    "which" to "face",
                    "op" to "open",
                    "facing" to "SELFIE",
                    "width" to width,
                    "height" to height,
                    "ms" to (Diag.ms() - startedMs),
                )
            } else {
                Diag.warn("camera", "which" to "face", "op" to "open", "error" to out, "ms" to (Diag.ms() - startedMs))
            }
            out
        } catch (e: SecurityException) {
            close()
            Diag.warn("camera", "which" to "face", "op" to "open", "error" to "permission")
            "the camera permission is not held"
        } catch (e: Exception) {
            close()
            Diag.warn("camera", "which" to "face", "op" to "open", "error" to e.toString())
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
        val f = latest.get()
        if (f == null) {
            // the 2 s frame wait the plan's `face.frames` row counts
            frameTimeouts.incrementAndGet()
            Diag.warn("face.timeout", "waited_ms" to 2_000, "delivered" to delivered.get())
            return ByteArray(0)
        }
        served.incrementAndGet()
        return f
    }

    /** Close the camera and report what it delivered.  Safe to call
     *  twice: the second does nothing. */
    @Synchronized
    fun close() {
        if (device != null || delivered.get() > 0 || frameTimeouts.get() > 0) {
            Diag.event(
                "face.frames",
                "delivered" to delivered.get(),
                "served" to served.get(),
                "frame_timeouts" to frameTimeouts.get(),
                "open_timed_out" to openTimedOut,
                "frame_bytes" to frameBytes,
                "width" to width,
                "height" to height,
            )
            Diag.event("camera", "which" to "face", "op" to "close")
        }
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
