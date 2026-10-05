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
import com.google.zxing.DecodeHintType
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
 * **What a lens added, which is why this file looks as it does.** The
 * camera half ran on two Galaxy S21+ phones from `af72cc1` onward, and
 * every guard here came out of that rather than out of a test: the first
 * code carried the whole 2 KB object and could not be read at arm's
 * length at all, which is what drove the kernel to cross it in 256-byte
 * parts; a second open while a read was live threw
 * `CameraAccessException -38`, so a read holds the device once; a session
 * that stalled never recovered, so a watchdog restarts it; and the frame
 * size is chosen per camera because the default was too small to resolve
 * a dense symbol. A scan still fails rather than returns the wrong bytes,
 * since a corrupted symbol fails its own error correction before ZXing
 * returns anything.
 *
 * The header said **NOT RUN ON HARDWARE** until 2026-10-05, which had
 * been false since `bc08f98`. A module header that outlives its subject
 * is worse than none: the phase 1 sweep documented this file and did not
 * read what was already in it (Reviewer2, sixth round).
 */
class QrCamera(private val context: Context) {

    /** Which camera a read uses: the invitation is scanned on the rear
     *  one, the optical exchange on the front. */
    enum class Facing {
        /** `LENS_FACING_BACK`. */
        REAR,
        /** `LENS_FACING_FRONT`. */
        SELFIE,
    }

    private companion object {
        /** Try harder: the dense first code at the edge of resolution is
         *  worth the extra pass per frame. */
        val DECODE_HINTS = mapOf<DecodeHintType, Any>(DecodeHintType.TRY_HARDER to true)
        /** A session that has read nothing for this long is restarted. A
         *  camera service on these phones comes up unhealthy at times, a
         *  minute of frames decoding nothing where a fresh session reads in
         *  a second; the person should not have to leave and come back. */
        const val STALL_MS = 20_000L
    }

    /**
     * The frame size: **as many pixels as the camera gives up to about
     * three megapixels**, chosen per camera from what it offers. The first
     * optical code is 173 modules across (`wire-format.md` §14.3.1), and at
     * arm's length a 720p frame put well under a pixel on each module,
     * which no decoder reads; a 1080p-class frame at a hand's length puts
     * two to three. Decoding a frame this size costs tens of milliseconds.
     */
    private var width = 1280
    private var height = 720
    private val maxPixels = 1920 * 1440

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
    /** Told, once, where the camera fails after it was asked for. */
    private var failed: ((String) -> Unit)? = null
    /** The symbol last delivered, so a redraw does not read it again. */
    private var lastRead: String? = null
    private var continuous = false
    private var found: ((ByteArray) -> Unit)? = null
    /** In continuous mode, the payload last delivered: the same code read
     *  again is not news. */
    private var lastPayload: ByteArray? = null
    /** When a code was last read, or the session opened: the watchdog's
     *  clock. */
    @Volatile private var lastReadMs: Long = 0
    private var manager: CameraManager? = null
    private var id: String? = null
    private var previewSurface: Surface? = null


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
    fun readOne(
        facing: Facing,
        preview: Surface?,
        which: String = "qr",
        failed: ((String) -> Unit)? = null,
        /** Keep reading after a symbol: every distinct payload is delivered,
         *  a payload equal to the last one is not, and the camera stays
         *  open until [close]. For an exchange whose codes change as the
         *  other side reads. */
        continuous: Boolean = false,
        found: (ByteArray) -> Unit,
    ): String? {
        // **One open per read.** A screen redraws for every note and step,
        // and each redraw asks for the scan again; a second open of the same
        // camera while the first is in flight has the service disconnect
        // the first, whose pending callback then throws off the UI thread
        // and takes the process with it. A read already running for this
        // symbol is left running.
        if (reading.get() && this.facing == facing && this.which == which) return null
        // a symbol this reader already delivered is not read again on a
        // redraw: the flow has it, and a second delivery after the step
        // moved on is what a late callback would hand a screen that is
        // no longer there
        if (which == lastRead) return null
        if (reading.get()) close()
        this.which = which
        this.facing = facing
        this.failed = failed
        this.continuous = continuous
        this.found = found
        lastPayload = null
        attempts.set(0)
        startedMs = Diag.ms()
        val manager = context.getSystemService(Context.CAMERA_SERVICE) as? CameraManager
            ?: return cameraRefused("this device exposes no camera service")
        val id = pick(manager, facing) ?: return cameraRefused("no ${facing.name.lowercase()} camera")
        size(manager, id)
        this.manager = manager
        this.id = id
        this.previewSurface = preview
        val t = HandlerThread("qr").also { it.start() }
        thread = t
        val h = Handler(t.looper)
        handler = h
        lastReadMs = Diag.ms()
        h.postDelayed(object : Runnable {
            override fun run() {
                if (!reading.get() || handler !== h) return
                if (Diag.ms() - lastReadMs >= STALL_MS) {
                    Diag.warn("camera", "which" to "qr", "op" to "restart", "facing" to facing, "attempts" to attempts.get(), "stalled_ms" to (Diag.ms() - lastReadMs))
                    restart()
                    return
                }
                h.postDelayed(this, 2_000)
            }
        }, 2_000)
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
                        lastReadMs = Diag.ms()
                        if (continuous) {
                            val last = lastPayload
                            if (last != null && last.contentEquals(it)) return@let
                            lastPayload = it
                            Diag.event(
                                "qr.read",
                                "which" to which,
                                "bytes" to it.size,
                                "facing" to facing,
                                "attempts" to attempts.get(),
                                "decode_ms" to (Diag.ms() - startedMs),
                            )
                            attempts.set(0)
                            startedMs = Diag.ms()
                            found(it)
                        } else if (reading.compareAndSet(true, false)) {
                            Diag.event(
                                "qr.read",
                                "which" to which,
                                "bytes" to it.size,
                                "facing" to facing,
                                "attempts" to attempts.get(),
                                "decode_ms" to (Diag.ms() - startedMs),
                            )
                            lastRead = which
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

    /** Close the stalled session and open a fresh one for the same read:
     *  the caller's callbacks stay as they are. */
    private fun restart() {
        val f = facing ?: return
        val w = which
        val fl = failed
        val cont = continuous
        val found = this.found ?: return
        val preview = previewSurface
        close()
        lastRead = null
        readOne(f, preview, w, fl, cont, found)
    }

    private fun cameraRefused(why: String): String {
        Diag.warn("camera", "which" to "qr", "op" to "open", "facing" to facing, "error" to why)
        return why
    }

    /** A failure on the camera's own thread: said, the camera given back,
     *  and the caller told once. Never a crash: the service's refusals
     *  (a disconnected device, a template it will not build) arrive here
     *  as exceptions in callbacks, outside any try the caller holds. */
    private fun failedOnThread(op: String, e: Throwable) {
        Timber.w(e, "qr camera %s", op)
        Diag.warn("camera", "which" to "qr", "op" to op, "facing" to facing, "error" to e.toString())
        val tell = failed
        failed = null
        close()
        tell?.invoke("the ${facing?.name?.lowercase() ?: ""} camera failed while $op: ${e.message ?: e.javaClass.simpleName}")
    }

    /** A frame to bytes, or null where there was no symbol in it. */
    private fun decode(y: ByteArray, rowStride: Int, w: Int, h: Int): ByteArray? {
        val source = PlanarYUVLuminanceSource(y, rowStride, h, 0, 0, w, h, false)
        return try {
            val text = QRCodeReader().decode(BinaryBitmap(HybridBinarizer(source)), DECODE_HINTS).text
            Optical.bytes(text)
        } catch (e: Exception) {
            // no symbol in this frame, or one that failed its own error
            // correction. Either way the next frame is the answer.
            null
        }
    }

    /** The largest YUV frame the camera offers within [maxPixels]. */
    private fun size(manager: CameraManager, id: String) {
        val map = manager.getCameraCharacteristics(id)
            .get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP) ?: return
        val best = map.getOutputSizes(ImageFormat.YUV_420_888)
            ?.filter { it.width * it.height <= maxPixels }
            ?.maxByOrNull { it.width * it.height } ?: return
        width = best.width
        height = best.height
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
                    try {
                    @Suppress("DEPRECATION")
                    camera.createCaptureSession(
                        surfaces,
                        object : CameraCaptureSession.StateCallback() {
                            override fun onConfigured(s: CameraCaptureSession) {
                                if (!reading.get()) {
                                    // closed, or replaced, while configuring
                                    runCatching { s.close() }
                                    return
                                }
                                session = s
                                try {
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
                                } catch (e: Exception) {
                                    failedOnThread("requesting frames", e)
                                }
                            }

                            override fun onConfigureFailed(s: CameraCaptureSession) {
                                failedOnThread("configuring", IllegalStateException("the session would not configure"))
                            }
                        },
                        h,
                    )
                    } catch (e: Exception) {
                        failedOnThread("opening a session", e)
                    }
                }

                override fun onDisconnected(camera: CameraDevice) {
                    if (device === camera) failedOnThread("open", IllegalStateException("the camera was disconnected"))
                    else runCatching { camera.close() }
                }

                override fun onError(camera: CameraDevice, error: Int) {
                    if (device === camera) failedOnThread("open", IllegalStateException("camera error $error"))
                    else runCatching { camera.close() }
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
