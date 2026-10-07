package com.comptus.fueros

import android.content.Context
import android.graphics.ImageFormat
import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraDevice
import android.hardware.camera2.CameraManager
import android.hardware.camera2.CaptureRequest
import android.hardware.camera2.CaptureResult
import android.hardware.camera2.TotalCaptureResult
import android.hardware.camera2.params.MeteringRectangle
import android.util.Range
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
         *  a second; the person should not have to leave and come back.
         *
         *  **Forty seconds, not twenty.** A colour frame's first read is
         *  slow — 7.3 s and 10.5 s on the two phones of the first colour
         *  run, against a monochrome median of 370 ms — and a restart
         *  inside that acquisition throws away the channels already
         *  separated. The trigger has to sit well clear of a read that is
         *  merely slow, and the author's standing instruction on this path
         *  is to be liberal throughout [author, 2026-10-06]. */
        const val STALL_MS = 40_000L

        /**
         * **One screen refresh, in hertz**: the frame rate whose interval
         * the exposure should not fall below, since a shutter shorter than
         * the panel's refresh catches it mid-draw. Sixty is one 16.7 ms
         * frame, and the phones these run on refresh at 60 or 120, so one
         * 60 Hz period integrates a whole frame on either ([briskest]).
         */
        const val SCREEN_HZ = 60

        /**
         * **How far under to expose a screen**, in stops. A code on a
         * phone screen is the highest contrast a camera is ever handed, so
         * a stop and a half down costs nothing it needs and keeps the
         * white modules from blooming over the black ([steps]).
         */
        const val DARKEN_EV = -1.5f
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
    /** The sensor's active array, for expressing a metering region in the
     *  coordinates `CONTROL_AF_REGIONS` wants. Null where the camera does
     *  not say, in which case no region is asked for. */
    private var active: android.graphics.Rect? = null
    /** How many AF and AE regions this camera will take: zero means it
     *  takes none and the request must not carry any. */
    private var afRegions = 0
    private var aeRegions = 0
    /** The target frame-rate range asked for, which caps the shutter
     *  ([briskest]). Null where the camera offers none. */
    private var fps: Range<Int>? = null
    /** Exposure compensation asked for, in the camera's own steps
     *  ([steps]); zero where it will not take any. */
    private var darken = 0
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
    /**
     * Where a colour frame's channels go: **every channel of one captured
     * frame in one delivery**, each the bytes and the channel that carried
     * them, since the channel is the part's index ([Polychrome]). Set
     * alongside [found], never instead: a frame is tried in colour and
     * then in luminance, so a monochrome code in front of a colour-reading
     * camera still reads.
     *
     * **One delivery a frame, not one a channel.** The caller decides from
     * how many channels separated whether colour works at all, and a
     * delivery per channel lets that decision run between the first
     * channel and the second: the first field run read all three channels
     * of a frame 15 ms apart and refused colour in the gap, on evidence
     * that was two channels short because the other two had not been
     * handed over yet [field run colour-1, 2026-10-06].
     */
    private var foundChannels: ((List<Pair<ByteArray, Int>>) -> Unit)? = null
    /** The three luminance planes a colour frame separates into, reused:
     *  eight megabytes at a 1920×1440 frame is not a per-frame allocation. */
    private var planes: Array<ByteArray>? = null
    /** What each channel last delivered, so a frame held up while the other
     *  side catches up is not read again. */
    private val lastChannel = arrayOfNulls<ByteArray>(Polychrome.CHANNELS)
    /** In continuous mode, the payload last delivered: the same code read
     *  again is not news. */
    private var lastPayload: ByteArray? = null
    /** When a code was last read, or the session opened: the watchdog's
     *  clock. */
    @Volatile private var lastReadMs: Long = 0
    private var manager: CameraManager? = null
    private var id: String? = null
    private var previewSurface: Surface? = null


    private val surveyed = java.util.concurrent.atomic.AtomicBoolean(false)
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
        /** Where a colour frame's three channels go, all of one captured
         *  frame in one delivery, where the caller is reading a polychrome
         *  exchange ([Polychrome]). */
        foundChannels: ((List<Pair<ByteArray, Int>>) -> Unit)? = null,
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
        this.foundChannels = foundChannels
        java.util.Arrays.fill(lastChannel, null)
        this.found = found
        lastPayload = null
        attempts.set(0)
        startedMs = Diag.ms()
        val manager = context.getSystemService(Context.CAMERA_SERVICE) as? CameraManager
            ?: return cameraRefused("this device exposes no camera service")
        val id = pick(manager, facing) ?: return cameraRefused("no ${facing.name.lowercase()} camera")
        size(manager, id)
        steering(manager, id)
        lens(manager, id)
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
                    // **colour first, where the caller is reading a
                    // polychrome exchange**: three channels separated out
                    // of the one frame, each its own symbol and its own
                    // part ([Polychrome]). A frame that yields nothing in
                    // colour falls through to the luminance read below, so
                    // a monochrome code in front of a colour-reading
                    // camera still reads and the fall back costs a frame
                    // and no state [author, 2026-10-06].
                    if (colour(image)) return@setOnImageAvailableListener
                    // the Y plane alone: ZXing wants luminance and a
                    // monochrome QR has no colour in it
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
        // the colour sink goes with it: a restart that dropped it would
        // leave a colour exchange reading nothing and read as colour
        // having failed
        val channel = this.foundChannels
        val preview = previewSurface
        close()
        lastRead = null
        readOne(f, preview, w, fl, cont, channel, found)
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

    /**
     * One frame read as three colour channels. True where a channel
     * delivered something, so the luminance read is not also tried.
     *
     * **The chroma is half-resolution and that is the risk the format was
     * built to measure** ([Polychrome]): a 4:2:0 frame carries one chroma
     * sample per 2×2 block, so whether three channels separate at module
     * scale is this camera's property and not the code's. A simulation of
     * the subsampling alone separates them from two pixels a module; what
     * it cannot simulate is this device's white balance.
     */
    private fun colour(image: android.media.Image): Boolean {
        val sink = foundChannels ?: return false
        if (image.planes.size < 3) return false
        val w = image.width
        val h = image.height
        val p = planes ?: Array(Polychrome.CHANNELS) { ByteArray(w * h) }.also { planes = it }
        if (p[0].size < w * h) return false
        val yp = image.planes[0]
        val up = image.planes[1]
        val vp = image.planes[2]
        val y = ByteArray(yp.buffer.remaining()).also { yp.buffer.get(it) }
        val u = ByteArray(up.buffer.remaining()).also { up.buffer.get(it) }
        val v = ByteArray(vp.buffer.remaining()).also { vp.buffer.get(it) }
        attempts.incrementAndGet()
        Polychrome.planesFromYuv(y, u, v, yp.rowStride, up.rowStride, up.pixelStride, w, h, p)
        // **the whole frame is separated before any of it is delivered**,
        // and every channel that decoded is delivered, held up or not: how
        // many planes came out of one capture is the evidence the caller
        // decides colour on ([foundChannels]), so it is the size of the
        // delivery and not something the caller has to count across them
        val separated = mutableListOf<Pair<ByteArray, Int>>()
        var fresh = 0
        for (ch in 0 until Polychrome.CHANNELS) {
            val bytes = Polychrome.decodePlane(p[ch], w, h) ?: continue
            // a plane that decoded is a healthy camera, whether or not the
            // part in it is news: the watchdog is asking about the camera
            lastReadMs = Diag.ms()
            separated.add(bytes to ch)
            val last = lastChannel[ch]
            // the same part held up while the other side catches up is
            // delivered but is not a read worth recording
            if (last != null && last.contentEquals(bytes)) continue
            lastChannel[ch] = bytes
            fresh++
            Diag.event(
                "qr.read",
                "which" to which,
                "bytes" to bytes.size,
                "facing" to facing,
                "channel" to ch,
                "attempts" to attempts.get(),
                "decode_ms" to (Diag.ms() - startedMs),
            )
        }
        if (separated.isEmpty()) return false
        // **how many planes this capture separated**: three is a camera
        // that can read the format, one or two is a camera that separates
        // some of them, and either way it is on the record for the run
        Diag.event(
            "qr.frame",
            "which" to which,
            "channels" to separated.size,
            "fresh" to fresh,
            "attempts" to attempts.get(),
        )
        sink(separated)
        if (fresh > 0) {
            attempts.set(0)
            startedMs = Diag.ms()
        }
        return true
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
    /**
     * **What the lens can actually do**, on the record once per open.
     *
     * The range the optical exchange reaches was being predicted from the
     * module's physical size, on a rule calibrated from one observation —
     * and the rule is wrong. Run `mono102` of 2026-10-07 put 2.14 mm
     * modules in front of these cameras, 2.2 times the 0.97 mm the rule
     * was fitted to, and read them at about 12 inches against the 18 the
     * denser code had managed: the range went *down* as the module grew.
     * At 12 inches a 2.14 mm module lands about eight pixels a module,
     * four times what a decoder needs, so **pixel resolution is not the
     * constraint and never was** [author, 2026-10-07, measured].
     *
     * The candidate that fits is the lens. A front camera with no
     * autofocus is fixed at roughly arm's length, and past that the blur
     * is a property of the optics that no module size answers. So the
     * figures that settle it go in the log: a minimum focus distance of
     * zero means fixed focus, the hyperfocal distance says where it is
     * sharp, and the available autofocus modes say whether there is
     * anything to drive.
     */
    private fun lens(manager: CameraManager, id: String) {
        // **every camera, once a process.** Which lens is open says
        // nothing about the one the next step will use, and the question
        // this answers is about the device and not about this read.
        if (surveyed.compareAndSet(false, true)) {
            val ids = try {
                manager.cameraIdList
            } catch (e: Exception) {
                emptyArray<String>()
            }
            for (other in ids) if (other != id) lensOf(manager, other, "survey")
        }
        lensOf(manager, id, which)
    }

    private fun lensOf(manager: CameraManager, id: String, label: String) {
        val c = try {
            manager.getCameraCharacteristics(id)
        } catch (e: Exception) {
            return
        }
        // dioptres: 0 is a lens that cannot be driven at all
        val minFocus = c.get(CameraCharacteristics.LENS_INFO_MINIMUM_FOCUS_DISTANCE)
        val hyper = c.get(CameraCharacteristics.LENS_INFO_HYPERFOCAL_DISTANCE)
        val afModes = c.get(CameraCharacteristics.CONTROL_AF_AVAILABLE_MODES)
        val focal = c.get(CameraCharacteristics.LENS_INFO_AVAILABLE_FOCAL_LENGTHS)
        val sensor = c.get(CameraCharacteristics.SENSOR_INFO_PHYSICAL_SIZE)
        val lensFacing = when (c.get(CameraCharacteristics.LENS_FACING)) {
            CameraCharacteristics.LENS_FACING_FRONT -> "SELFIE"
            CameraCharacteristics.LENS_FACING_BACK -> "REAR"
            else -> "OTHER"
        }
        Diag.event(
            "camera.lens",
            "which" to label,
            "id" to id,
            "facing" to lensFacing,
            // a dioptre is 1/m, so the nearest it focuses is 1/this metres
            "min_focus_dioptre" to (minFocus?.toString() ?: "none"),
            "nearest_mm" to (minFocus?.let { if (it > 0f) (1000f / it).toInt() else -1 } ?: -1),
            "hyperfocal_dioptre" to (hyper?.toString() ?: "none"),
            "hyperfocal_mm" to (hyper?.let { if (it > 0f) (1000f / it).toInt() else -1 } ?: -1),
            "fixed_focus" to (minFocus != null && minFocus == 0f),
            "af_modes" to (afModes?.joinToString("|") ?: "none"),
            "focal_mm" to (focal?.joinToString("|") ?: "none"),
            "sensor_mm" to (sensor?.let { "${it.width}x${it.height}" } ?: "none"),
        )
    }

    /**
     * **Whether the lens ever found the code**, which was invisible: the
     * repeating request was made with a null callback, so no capture
     * result was ever looked at and a camera focused on the wall behind
     * the counterparty was indistinguishable from one focused on their
     * screen.
     *
     * Reported on a change of autofocus state and otherwise about once a
     * second, so a run carries the lens's own account of itself without
     * a line a frame. `focus_mm` is where the lens is; `af` is 0 inactive,
     * 1 passive scan, 2 passive focused, 3 active scan, 4 locked,
     * 5 not focused, 6 passive unfocused.
     */
    private val focusWatch = object : CameraCaptureSession.CaptureCallback() {
        private var lastState: Int? = null
        private var lastSaidMs = 0L

        override fun onCaptureCompleted(
            session: CameraCaptureSession,
            request: CaptureRequest,
            result: TotalCaptureResult,
        ) {
            if (!reading.get()) return
            val af = result.get(CaptureResult.CONTROL_AF_STATE)
            val now = Diag.ms()
            val changed = af != lastState
            if (!changed && now - lastSaidMs < 1_000) return
            lastState = af
            lastSaidMs = now
            val d = result.get(CaptureResult.LENS_FOCUS_DISTANCE)
            Diag.event(
                "camera.focus",
                "which" to which,
                "facing" to facing,
                "af" to (af ?: -1),
                "ae" to (result.get(CaptureResult.CONTROL_AE_STATE) ?: -1),
                // dioptres again: 0 is focused at infinity
                "focus_mm" to (d?.let { if (it > 0f) (1000f / it).toInt() else -1 } ?: -1),
                "iso" to (result.get(CaptureResult.SENSOR_SENSITIVITY) ?: -1),
                "exposure_us" to ((result.get(CaptureResult.SENSOR_EXPOSURE_TIME) ?: 0L) / 1000L),
                "attempts" to attempts.get(),
            )
        }
    }

    /** What the camera will let the caller steer: the active array the
     *  regions are expressed in, and how many of each it accepts. */
    private fun steering(manager: CameraManager, id: String) {
        val c = try {
            manager.getCameraCharacteristics(id)
        } catch (e: Exception) {
            return
        }
        active = c.get(CameraCharacteristics.SENSOR_INFO_ACTIVE_ARRAY_SIZE)
        afRegions = c.get(CameraCharacteristics.CONTROL_MAX_REGIONS_AF) ?: 0
        aeRegions = c.get(CameraCharacteristics.CONTROL_MAX_REGIONS_AE) ?: 0
        fps = briskest(c.get(CameraCharacteristics.CONTROL_AE_AVAILABLE_TARGET_FPS_RANGES))
        darken = steps(
            c.get(CameraCharacteristics.CONTROL_AE_COMPENSATION_STEP),
            c.get(CameraCharacteristics.CONTROL_AE_COMPENSATION_RANGE),
        )
    }

    /**
     * **The frame rate that caps the shutter**, which is the only lever on
     * exposure time that does not take exposure away from the camera
     * altogether: a frame cannot last longer than its own interval, so a
     * floor under the frame rate is a ceiling over the exposure.
     *
     * Run `aimed-1` of 2026-10-07 metered **42 to 66 ms** — a quarter to a
     * sixteenth of a second, handheld, on a target whose features are two
     * millimetres. Auto exposure spent the light the metering region gave
     * it on lowering the sensitivity instead of shortening the shutter,
     * which is right for a photograph and wrong for this.
     *
     * **Sixty, and not higher, because the subject is a screen.** The
     * exposure has to integrate at least one whole refresh or it catches
     * the panel mid-draw; one 60 Hz frame is 16.7 ms, which is also short
     * enough to hold a hand still to within a module. So the choice is the
     * briskest range the camera offers whose floor does not exceed 60, and
     * the slowest it offers if every range is faster than that.
     */
    private fun briskest(ranges: Array<Range<Int>>?): Range<Int>? {
        val all = ranges?.takeIf { it.isNotEmpty() } ?: return null
        return all.filter { it.lower <= SCREEN_HZ }.maxWithOrNull(
            compareBy({ it.lower }, { it.upper }),
        ) ?: all.minByOrNull { it.lower }
    }

    /**
     * **How far to underexpose**, in the camera's own compensation steps.
     *
     * A phone screen is self-luminous and a code on it is the highest
     * contrast a camera will ever be handed, so deliberate underexposure
     * costs nothing it needs and stops the white modules blooming over the
     * black ones — which is the failure a long exposure on a bright screen
     * produces. It also pushes auto exposure towards the shutter rather
     * than the sensitivity.
     */
    private fun steps(step: android.util.Rational?, range: Range<Int>?): Int {
        val ev = step?.toFloat()?.takeIf { it > 0f } ?: return 0
        val want = Math.round(DARKEN_EV / ev)
        val r = range ?: return 0
        return want.coerceIn(r.lower, r.upper)
    }

    /**
     * **The middle of the frame, which is where the other phone is.**
     *
     * Continuous autofocus and auto exposure with no region given are
     * free to choose anything in the scene, and at arm's length the
     * counterparty's screen *is* most of the scene so they choose it by
     * accident. At three feet it is a small bright rectangle in a room:
     * the lens focuses past it and the metering averages it away against
     * the room, so the white modules bloom into the black ones. Neither
     * failure is answered by a bigger module, which is why enlarging the
     * module twofold bought no range at all [measured, 2026-10-07].
     *
     * A third of the frame each way, centred — the area the aim band on
     * screen already tells the person to fill.
     */
    private fun middle(): Array<MeteringRectangle>? {
        val a = active ?: return null
        val w = a.width() / 3
        val h = a.height() / 3
        return arrayOf(
            MeteringRectangle(
                a.left + w,
                a.top + h,
                w,
                h,
                MeteringRectangle.METERING_WEIGHT_MAX,
            ),
        )
    }

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
                        "took_ms" to (Diag.ms() - startedMs),
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
                                    // **the middle of the frame is the
                                    // subject** ([middle]): a camera given
                                    // no region focuses and meters on
                                    // whatever it likes, which at any
                                    // distance where the other phone is
                                    // not most of the scene is not the
                                    // other phone
                                    val region = middle()
                                    if (region != null && afRegions > 0) {
                                        b.set(CaptureRequest.CONTROL_AF_REGIONS, region)
                                    }
                                    if (region != null && aeRegions > 0) {
                                        b.set(CaptureRequest.CONTROL_AE_REGIONS, region)
                                    }
                                    // **a floor under the frame rate is a
                                    // ceiling over the shutter** ([fps]),
                                    // and the screen is underexposed on
                                    // purpose ([darken])
                                    fps?.let { b.set(CaptureRequest.CONTROL_AE_TARGET_FPS_RANGE, it) }
                                    if (darken != 0) {
                                        b.set(CaptureRequest.CONTROL_AE_EXPOSURE_COMPENSATION, darken)
                                    }
                                    Diag.event(
                                        "camera",
                                        "which" to "qr",
                                        "op" to "request",
                                        "template" to "preview",
                                        "af" to "continuous_picture",
                                        "exposure" to "auto",
                                        "antibanding" to "default",
                                        "af_regions" to afRegions,
                                        "ae_regions" to aeRegions,
                                        "region" to (region?.get(0)?.rect?.toShortString() ?: "none"),
                                        "fps" to (fps?.toString() ?: "none"),
                                        "darken_steps" to darken,
                                    )
                                    s.setRepeatingRequest(b.build(), focusWatch, h)
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
