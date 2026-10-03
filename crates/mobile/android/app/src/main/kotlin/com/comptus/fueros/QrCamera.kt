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
import com.google.zxing.BinaryBitmap
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader
import timber.log.Timber

/**
 * The camera, reading one QR (`wire-format.md` §14.3.1).
 *
 * **Two cameras and two moments**, which is the author's ruling on the
 * handshake: the **bootstrap** QR at D1 is read with the REAR camera, one
 * direction, before either party has accepted; the **anchor** exchange at
 * D2 is mutual and read with the SELFIE cameras, both phones turned to face
 * each other. [Facing] is which, and the caller says which moment it is in.
 *
 * **What crosses out of here is bytes.** This file decodes a symbol and
 * hands over what was in it; whether those bytes are an
 * `OpticalContribution`, a `TranscriptConfirm` or nothing of the sort is
 * the kernel's to say, and it says so by refusing them.
 *
 * **NOT RUN ON HARDWARE.** It compiles, and the payload path it feeds is
 * round-tripped in `OpticalTest`; the camera half has never seen a lens.
 * What a lens adds is blur, perspective and exposure — the reasons a scan
 * fails rather than returns the wrong bytes, since a corrupted symbol
 * fails its own error correction before ZXing returns anything.
 */
class QrCamera(private val context: Context) {

    enum class Facing { REAR, SELFIE }

    /** The preview size asked for: enough modules to resolve, no more. */
    private val width = 1280
    private val height = 720

    private var thread: HandlerThread? = null
    private var handler: Handler? = null
    private var device: CameraDevice? = null
    private var session: CameraCaptureSession? = null
    private var reader: ImageReader? = null
    private val reading = java.util.concurrent.atomic.AtomicBoolean(false)

    /** For the `qr.read` event: which symbol this read is for, the frames
     *  tried, and when the read began. */
    private var which: String = "?"
    private var facing: Facing? = null
    private val attempts = java.util.concurrent.atomic.AtomicInteger(0)
    private var startedMs: Long = 0

    /**
     * Read one QR and stop. `found` is called once, on the camera's own
     * thread, with the bytes the symbol carried; `preview` is where the
     * person sees what they are pointing at.
     *
     * A permission this shell does not hold is a refusal, not a crash: the
     * caller is told and the person is asked for it elsewhere.
     */
    fun readOne(facing: Facing, preview: Surface?, which: String = "qr", found: (ByteArray) -> Unit): String? {
        this.which = which
        this.facing = facing
        attempts.set(0)
        startedMs = Diag.ms()
        val manager = context.getSystemService(Context.CAMERA_SERVICE) as? CameraManager
            ?: return cameraRefused("this device exposes no camera service")
        val id = pick(manager, facing) ?: return cameraRefused("no ${facing.name.lowercase()} camera")
        val t = HandlerThread("qr").also { it.start() }
        thread = t
        val h = Handler(t.looper)
        handler = h
        val r = ImageReader.newInstance(width, height, ImageFormat.YUV_420_888, 2)
        reader = r
        reading.set(true)
        r.setOnImageAvailableListener({ ir ->
            val image = ir.acquireLatestImage() ?: return@setOnImageAvailableListener
            try {
                if (reading.get()) {
                    // the Y plane alone: ZXing wants luminance and a QR has
                    // no colour in it
                    val y = image.planes[0]
                    val row = y.rowStride
                    val buf = y.buffer
                    val bytes = ByteArray(buf.remaining())
                    buf.get(bytes)
                    attempts.incrementAndGet()
                    decode(bytes, row, image.width, image.height)?.let {
                        if (reading.compareAndSet(true, false)) {
                            Diag.event(
                                "qr.read",
                                "which" to which,
                                "bytes" to it.size,
                                "facing" to facing,
                                "attempts" to attempts.get(),
                                "decode_ms" to (Diag.ms() - startedMs),
                            )
                            found(it)
                            close()
                        }
                    }
                }
            } catch (e: Exception) {
                Timber.w(e, "qr frame")
                Diag.warn("camera", "which" to "qr", "op" to "frame", "error" to e.toString())
            } finally {
                image.close()
            }
        }, h)
        return try {
            open(manager, id, r, preview, h)
            null
        } catch (e: SecurityException) {
            close()
            cameraRefused("the camera permission is not held")
        } catch (e: Exception) {
            close()
            cameraRefused("the camera would not open: $e")
        }
    }

    private fun cameraRefused(why: String): String {
        Diag.warn("camera", "which" to "qr", "op" to "open", "facing" to facing, "error" to why)
        return why
    }

    /** A frame to bytes, or null where there was no symbol in it. */
    private fun decode(y: ByteArray, rowStride: Int, w: Int, h: Int): ByteArray? {
        val source = PlanarYUVLuminanceSource(y, rowStride, h, 0, 0, w, h, false)
        return try {
            val text = QRCodeReader().decode(BinaryBitmap(HybridBinarizer(source))).text
            Optical.bytes(text)
        } catch (e: Exception) {
            // no symbol in this frame, or one that failed its own error
            // correction. Either way the next frame is the answer.
            null
        }
    }

    private fun pick(manager: CameraManager, facing: Facing): String? {
        val want = when (facing) {
            Facing.REAR -> CameraCharacteristics.LENS_FACING_BACK
            Facing.SELFIE -> CameraCharacteristics.LENS_FACING_FRONT
        }
        return manager.cameraIdList.firstOrNull { id ->
            manager.getCameraCharacteristics(id)
                .get(CameraCharacteristics.LENS_FACING) == want
        }
    }

    private fun open(
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
                    Diag.event(
                        "camera",
                        "which" to "qr",
                        "op" to "open",
                        "facing" to facing,
                        "width" to width,
                        "height" to height,
                        "ms" to (Diag.ms() - startedMs),
                    )
                    val surfaces = listOfNotNull(reader.surface, preview)
                    @Suppress("DEPRECATION")
                    camera.createCaptureSession(
                        surfaces,
                        object : CameraCaptureSession.StateCallback() {
                            override fun onConfigured(s: CameraCaptureSession) {
                                session = s
                                val b = camera.createCaptureRequest(
                                    CameraDevice.TEMPLATE_PREVIEW,
                                )
                                surfaces.forEach { b.addTarget(it) }
                                b.set(
                                    CaptureRequest.CONTROL_AF_MODE,
                                    CaptureRequest.CONTROL_AF_MODE_CONTINUOUS_PICTURE,
                                )
                                // the request as set: no exposure or
                                // anti-banding choice is made here yet
                                Diag.event(
                                    "camera",
                                    "which" to "qr",
                                    "op" to "request",
                                    "template" to "preview",
                                    "af" to "continuous_picture",
                                    "exposure" to "auto",
                                    "antibanding" to "default",
                                )
                                s.setRepeatingRequest(b.build(), null, h)
                            }

                            override fun onConfigureFailed(s: CameraCaptureSession) {
                                Timber.w("qr session would not configure")
                                Diag.warn("camera", "which" to "qr", "op" to "configure", "error" to "failed")
                                close()
                            }
                        },
                        h,
                    )
                }

                override fun onDisconnected(camera: CameraDevice) {
                    Diag.warn("camera", "which" to "qr", "op" to "disconnected")
                    close()
                }

                override fun onError(camera: CameraDevice, error: Int) {
                    Timber.w("qr camera error %d", error)
                    Diag.warn("camera", "which" to "qr", "op" to "error", "error" to error)
                    close()
                }
            },
            h,
        )
    }

    /** Stop and give the camera back. Safe to call twice. */
    fun close() {
        reading.set(false)
        if (device != null) {
            Diag.event("camera", "which" to "qr", "op" to "close", "attempts" to attempts.get())
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
    }
}
