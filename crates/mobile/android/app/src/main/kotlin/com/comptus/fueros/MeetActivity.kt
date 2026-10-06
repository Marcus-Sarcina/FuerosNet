package com.comptus.fueros

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.Typeface
import android.os.Bundle
import android.widget.Button
import android.widget.CheckBox
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.graphics.SurfaceTexture
import android.view.Surface
import android.view.TextureView
import android.widget.FrameLayout

/**
 * Meet: the ceremony flow (screens sheet section D). One Activity walks the
 * whole sequence, because a ceremony is one continuous flow and not a set
 * of places you navigate between. The state lives in [Kernel]'s [Meet], so
 * a recreation mid-flow rejoins it.
 *
 * **What this build carries and what it cannot.** The flow, its front-
 * loaded brief, its hands-off phase and its review-before-sign gate are all
 * here and enforced by [Meet]. **D1 through D4 are wired to hardware**: the
 * bootstrap and anchor QRs on the rear and selfie cameras, the intent over
 * a Bluetooth LE bearer, the proximity tap over NFC (with the optical pass
 * D2 already made), and the guided capture on the selfie camera, every
 * radio and camera half compiling against the platform's API and **none of
 * it run on a device**, which the files behind each say at the top. **D5
 * through D7 run on the kernel's courier**: the request to witness, the
 * queries and their consent, the gathered responses, the body and the
 * record cross the end-to-end path (`wire-format.md` §7.10.1), the kernel
 * reviews and signs as the body arrives, and the flow moves on what the
 * kernel reports. The screens from D5 on show and let the person stop;
 * nothing there is a control over the protocol.
 */
class MeetActivity : Activity() {

    private lateinit var body: LinearLayout
    private lateinit var scroll: ScrollView
    private var camera: QrCamera? = null
    /** The selfie camera's view, kept across redraws so the session it
     *  feeds is not torn down with the screen: what the person holding the
     *  other phone steers by, since this one faces away from its user. */
    private var preview: TextureView? = null
    private var previewSurface: Surface? = null
    /** Where the preview lives: a fixed child of the root below the
     *  scrolling screen, so a redraw never detaches the TextureView (a
     *  detached and re-attached TextureView has no layer to draw and
     *  crashes the next frame). Hidden except at the optical step. */
    private lateinit var previewHost: FrameLayout

    /** Hands-off steps started this screen's life: so a redraw rejoins a
     *  running step rather than starting it again. */
    private val started = java.util.Collections.synchronizedSet(mutableSetOf<String>())

    private val sink = object : Meet.Ui {
        override fun render() = runOnUiThread { redraw() }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        body = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        scroll = ScrollView(this).apply {
            addView(body)
            layoutParams = LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f)
        }
        // the preview: a full-width band showing the middle of the camera's
        // view, the rest cropped above and below [author, 2026-10-04]; it
        // sits above the scrolling screen so whatever the screen cannot fit
        // is the band and never the code
        val full = resources.displayMetrics.widthPixels - 96
        val band = full / 3
        val tv = TextureView(this).also { preview = it }
        tv.surfaceTextureListener = object : TextureView.SurfaceTextureListener {
            override fun onSurfaceTextureAvailable(st: SurfaceTexture, width: Int, height: Int) {
                // a 4:3 buffer the camera will take as a preview target
                st.setDefaultBufferSize(1280, 960)
                previewSurface = Surface(st)
                Diag.event("camera", "which" to "qr", "op" to "preview", "state" to "available")
                redraw()
            }
            override fun onSurfaceTextureSizeChanged(st: SurfaceTexture, width: Int, height: Int) {}
            override fun onSurfaceTextureUpdated(st: SurfaceTexture) {}
            override fun onSurfaceTextureDestroyed(st: SurfaceTexture): Boolean {
                previewSurface?.release()
                previewSurface = null
                return true
            }
        }
        previewHost = FrameLayout(this).apply {
            setBackgroundColor(Color.DKGRAY)
            clipChildren = true
            clipToPadding = true
            // the camera's 4:3 frame drawn at full width, centred, and
            // clipped to the band: the middle third of what it sees
            addView(
                tv,
                FrameLayout.LayoutParams(full, full * 4 / 3).apply {
                    gravity = android.view.Gravity.CENTER
                },
            )
            layoutParams = LinearLayout.LayoutParams(full, band).apply { bottomMargin = 12 }
            visibility = android.view.View.GONE
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(48, 48, 48, 48)
            addView(previewHost)
            addView(scroll)
        }
        // the system bars' insets, applied by hand: Android 15 draws edge
        // to edge and does not honour fitsSystemWindows on a plain layout,
        // which put the first code under the status bar
        root.setOnApplyWindowInsetsListener { v, insets ->
            val bars = insets.getInsets(android.view.WindowInsets.Type.systemBars())
            v.setPadding(48 + bars.left, 48 + bars.top, 48 + bars.right, 48 + bars.bottom)
            insets
        }
        setContentView(root)
    }

    override fun onStart() {
        super.onStart()
        Consent.host(this)
        // the NFC reader side needs a foreground activity (D3); this is it
        ProximityChannels.host(this)
        Kernel.meet()?.bind(sink) ?: redraw()
    }

    override fun onStop() {
        Consent.release(this)
        ProximityChannels.release(this)
        Kernel.meet()?.unbind(sink)
        // the camera goes back the moment this screen stops: holding one
        // behind a screen nobody is looking at is a camera nobody consented
        // to
        camera?.close()
        camera = null
        super.onStop()
    }

    private fun redraw() {
        body.removeAllViews()
        previewHost.visibility = android.view.View.GONE
        val m = Kernel.meet()
        if (m == null) {
            entry()
            report()
            return
        }
        title(
            when (m.step()) {
                Meet.Step.INTENT -> "Meet ${m.counterpartyName}"
                Meet.Step.BRIEF -> "Before you begin"
                Meet.Step.OPTICAL -> "Exchanging (1/3)"
                Meet.Step.PROXIMITY -> "Ranging (2/3)"
                Meet.Step.CAPTURE -> "Capturing (3/3)"
                Meet.Step.VERIFIERS -> "Verifiers"
                Meet.Step.REVIEW -> "Review and sign"
                Meet.Step.DONE -> "Done"
                Meet.Step.STOPPED -> "Stopped"
            },
        )
        when (m.step()) {
            Meet.Step.INTENT -> intent(m)
            Meet.Step.BRIEF -> brief(m)
            Meet.Step.OPTICAL -> optical(m)
            Meet.Step.PROXIMITY -> proximity(m)
            Meet.Step.CAPTURE -> capture(m)
            Meet.Step.VERIFIERS -> verifiers(m)
            Meet.Step.REVIEW -> review(m)
            Meet.Step.DONE -> done(m)
            Meet.Step.STOPPED -> stopped(m)
        }
        logLines(m)
        report()
    }

    /** The Report action (`Robot/field-test-diagnostics.md`, section 4):
     *  the run's events and a header, zipped and offered to the share
     *  sheet. Fieldtest flavour only; the releasable flavour has no button
     *  and nothing it would send. */
    private fun report() {
        if (!BuildConfig.FIELD_TEST) return
        button("Send diagnostics (field test)") { Report.send(this) }
    }

    /**
     * Every permission asked for is an event, and so is each answer
     * (`Robot/field-test-diagnostics.md`, section 3.6, `permission`).
     */
    private fun ask(code: Int, vararg permissions: String) {
        for (p in permissions) {
            Diag.event("permission", "which" to short(p), "state" to "requested")
        }
        requestPermissions(arrayOf(*permissions), code)
    }

    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        for (i in permissions.indices) {
            val granted = grantResults.getOrNull(i) == PackageManager.PERMISSION_GRANTED
            Diag.event("permission", "which" to short(permissions[i]), "state" to if (granted) "granted" else "denied")
        }
        redraw()
    }

    private fun short(permission: String) = permission.substringAfterLast('.').lowercase()

    private fun held(permission: String) =
        checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED

    /** The three Bluetooth permissions the bearer needs at API 31 and up;
     *  asked for before the hands-off phase, since a radio refused there
     *  would only be caught, not asked for. */
    private val bluetooth = arrayOf(
        Manifest.permission.BLUETOOTH_SCAN,
        Manifest.permission.BLUETOOTH_ADVERTISE,
        Manifest.permission.BLUETOOTH_CONNECT,
    )

    // ---- the entry, before a ceremony is live --------------------------

    private fun entry() {
        val peer = Kernel.peerKey()
        if (peer == null) {
            para("A ceremony is with someone you are provisioned to. None yet.")
            return
        }
        // D1a, the initiator's dialogue, as the author specified it: a
        // regular meeting is the DEFAULT, with the backup checkbox beside
        // it, and patronage a separate option that then asks the direction.
        para("A meeting establishes a presence record with the other person, in person. What this meeting is for is chosen now and cannot change once it begins.")
        heading("Regular meeting")
        val backup = CheckBox(this).apply {
            text = "Ask this person to backup my user data"
            setTextColor(Color.DKGRAY)
        }
        body.addView(backup)
        button("Show my code") {
            start(Meet.Kind(Meet.Adopt.NONE, backup.isChecked), Meet.Role.INITIATOR)
        }
        heading("Patronage")
        para("An adoption moves authority. What it costs is listed before either of you agrees.")
        button("I will be the Patron") {
            start(Meet.Kind(Meet.Adopt.THEM_UNDER_ME), Meet.Role.INITIATOR)
        }
        button("I will be the Client") {
            start(Meet.Kind(Meet.Adopt.ME_UNDER_THEM), Meet.Role.INITIATOR)
        }
        heading("Or join theirs")
        para("The other person has shown you a code. Their choice of transaction is in it, and the next screen is where you accept or refuse it.")
        button("Scan a QR code") { start(Meet.Kind(), Meet.Role.RESPONDER) }
    }

    private fun start(kind: Meet.Kind, role: Meet.Role) {
        Kernel.startMeet(kind, role)?.bind(sink) ?: para("could not begin: the kernel is not running")
        redraw()
    }

    // ---- D1 intent -----------------------------------------------------

    private fun intent(m: Meet) {
        when (m.role) {
            Meet.Role.INITIATOR -> {
                para("Hold this up for ${m.counterpartyName} to scan with their REAR camera. It carries who you are and what kind of meeting this is — nothing of the meeting's own anchor, which comes later and goes both ways.")
                val code = Kernel.bootstrap()
                if (code == null) {
                    para("The code needs the kernel's identifier, which this device has not got yet.")
                } else {
                    qr(code, "bootstrap")
                }
                para("When they have scanned it, both of you will be shown what is about to happen.")
                button("They have scanned it") { m.crossBootstrap() }
            }
            Meet.Role.RESPONDER -> {
                para("Point the back of your phone at ${m.counterpartyName}'s screen. Their code carries who they are and the kind of meeting they chose; the next screen is where you accept or refuse it.")
                aim("The band at the top is the middle of what the rear camera sees: put their code in it.")
                scan(QrCamera.Facing.REAR, "bootstrap") { bytes, _ ->
                    // the bootstrap is the shell's own object and carries no
                    // anchor, so the shell reads it (`wire-format.md` §14.3
                    // fixes the ANCHORED objects and this is not one)
                    runOnUiThread {
                        // a read that lands after the step moved on is not
                        // this screen's any more
                        if (m.step() != Meet.Step.INTENT) return@runOnUiThread
                        val read = Kernel.takeBootstrap(bytes)
                        if (read == null) m.crossBootstrap() else m.stop(read)
                    }
                }
            }
        }
    }

    /** A QR on the screen, as large as the layout allows. What it reads
     *  at is **its module as a fraction of its own width**, never a pixel
     *  count (`OpticalExchange.CHUNK`): about `450 / modules` times the
     *  code's width, so the diagnostics carry the module's millimetres and
     *  the symbol's modules rather than the pixel scale alone. */
    /**
     * **A colour frame: three parts in one image** ([Polychrome]), drawn
     * as wide as the content area like any other code. The three symbols
     * share a module grid, so what the screen shows is one symbol's worth
     * of modules carrying three parts.
     */
    private fun qrColour(frames: List<ByteArray>, which: String) {
        if (frames.isEmpty()) return
        // a short frame at the object's end repeats its last part rather
        // than leaving a channel blank: a blank channel is a channel the
        // reader reports as lost, which would read as colour failing
        val three = (0 until Polychrome.CHANNELS).map { frames.getOrElse(it) { frames.last() } }
        val modules = Optical.matrix(three[0]).width
        val avail = if (body.width > 0) body.width else resources.displayMetrics.widthPixels - 96 - 48
        val scale = maxOf(1, (avail - 8) / modules)
        val (px, w) = Polychrome.compose(three, scale)
        val dpi = resources.displayMetrics.xdpi
        val moduleMm = if (dpi > 0f) scale.toFloat() / dpi * 25.4f else 0f
        Diag.event(
            "qr.shown", "which" to which, "bytes" to three.sumOf { it.size },
            "modules" to modules, "scale" to scale, "channels" to Polychrome.CHANNELS,
            "module_mm" to String.format("%.2f", moduleMm),
            "reads_at_mm" to (450f * moduleMm).toInt(),
        )
        show(px, w)
    }

    private fun qr(bytes: ByteArray, which: String) {
        val m = Optical.matrix(bytes)
        // **As wide as the content area, always**: what a code reads at is
        // its module's physical size, which is this width divided by the
        // symbol's modules, so every pixel of width is range
        // [author, 2026-10-04]. The 2 KB object is 43 parts of 37 modules
        // since 2026-10-06 (`OpticalExchange.CHUNK` carries the measured
        // trade); it used to be one symbol of 177, which is what the
        // chunking replaced. The area is the body's own width once laid
        // out, never the screen's: a code wider than its parent is clipped
        // at the right, quiet zone and modules, and reads nowhere
        val avail = if (body.width > 0) body.width else resources.displayMetrics.widthPixels - 96 - 48
        val scale = maxOf(1, (avail - 8) / m.width)
        // **The module's physical size, and the range it earns.** Pixels
        // are how this is drawn and say nothing about what reads it: a
        // module is a length on glass, and `xdpi` is what turns the one
        // into the other. Reported so a run on another phone compares
        // [author, 2026-10-06]; `450 / modules` times the code's own width
        // is the calibration the field runs give (`OpticalExchange.CHUNK`).
        val dpi = resources.displayMetrics.xdpi
        val moduleMm = if (dpi > 0f) (m.width * scale).toFloat() / m.width / dpi * 25.4f else 0f
        Diag.event(
            "qr.shown", "which" to which, "bytes" to bytes.size, "modules" to m.width,
            "scale" to scale,
            "module_mm" to String.format("%.2f", moduleMm),
            "reads_at_mm" to (450f * moduleMm).toInt(),
        )
        val w = m.width * scale
        val px = IntArray(w * w)
        for (y in 0 until w) {
            for (x in 0 until w) {
                px[y * w + x] = if (m.get(x / scale, y / scale)) Color.BLACK else Color.WHITE
            }
        }
        show(px, w)
    }

    /** Pixels into the view, which the two renderers share. */
    private fun show(px: IntArray, w: Int) {
        body.addView(
            ImageView(this).apply {
                setImageBitmap(Bitmap.createBitmap(px, w, w, Bitmap.Config.ARGB_8888))
                layoutParams = LinearLayout.LayoutParams(w, w).apply { topMargin = 24 }
            },
        )
    }

    /**
     * Read one QR with the named camera. The permission is asked for here
     * and a refusal stops the ceremony with a reason rather than silently:
     * a camera this shell does not hold is a meeting it cannot carry.
     */
    /**
     * **What the selfie camera sees**, under the code, for the person
     * holding the other phone to aim by [author, 2026-10-04]: this phone
     * faces away from its user, so its screen is the other holder's
     * instrument. The view and its surface outlive the redraws, and the
     * camera session takes the surface as a second target; the first read
     * waits for the surface where it is not up yet.
     */
    private fun aim(line: String = "The band at the top is the middle of what this phone's selfie camera sees. The other person steers by it.") {
        previewHost.visibility = android.view.View.VISIBLE
        para(line)
    }

    private fun scan(
        facing: QrCamera.Facing,
        which: String,
        continuous: Boolean = false,
        /** Read the frame as three colour channels as well as in
         *  luminance ([Polychrome]); the channel reaches `found` as its
         *  second argument and is a part's index within its frame. */
        channels: Boolean = false,
        found: (ByteArray, Int) -> Unit,
    ) {
        if (!held(Manifest.permission.CAMERA)) {
            para("This step needs the camera. Nothing is read until you allow it.")
            button("Allow the camera") { ask(1, Manifest.permission.CAMERA) }
            return
        }
        // a read waits for its preview surface, which arrives on the next
        // redraw
        val surface = previewSurface ?: run { para("Starting the camera…"); return }
        para("Scanning…")
        val cam = camera ?: QrCamera(this).also { camera = it }
        // a camera that fails after it was asked for stops the meeting with
        // the reason; a redraw while a read is running does not reopen it
        val onFailed: (String) -> Unit = { why ->
            runOnUiThread { Kernel.meet()?.stop(why) ?: redraw() }
        }
        val channelSink: ((ByteArray, Int) -> Unit)? =
            if (channels) { bytes, ch -> found(bytes, ch) } else null
        cam.readOne(facing, surface, which, onFailed, continuous, channelSink) { bytes -> found(bytes, 0) }
            ?.let { why -> para(why) }
    }

    // ---- D1.5 the brief: everything front-loaded -----------------------

    private fun brief(m: Meet) {
        para("This is what is about to happen. Both of you are being shown it, and both of you answer.")
        heading(
            when (m.adopt) {
                Meet.Adopt.NONE -> "A regular meeting with ${m.counterpartyName}"
                Meet.Adopt.ME_UNDER_THEM -> "${m.counterpartyName} as your patron"
                Meet.Adopt.THEM_UNDER_ME -> "You as ${m.counterpartyName}'s patron"
            },
        )
        // every item the model says this ceremony earns, and no reassurance
        // it has not earned
        m.brief().forEach { para("• ${it.text}") }
        heading("What may be weak in this meeting")
        para("• Nominated witnesses: none, until your horizon can offer them. The record carries that it had none.")
        para("• Verifiers and proximity are weighed, not required; a thin meeting is honest, not malformed.")
        if (bluetooth.any { !held(it) }) {
            // the bearer that carries the intent is Bluetooth LE; without
            // the permissions the exchange stops at the radio, said here
            // where the person can still act rather than caught there
            para("The intent crosses over Bluetooth, which this app is not yet allowed to use. Without it the meeting will stop at the exchange.")
            button("Allow Bluetooth") { ask(3, *bluetooth) }
        }
        button("Accept — begin") { m.accept() }
        button("Refuse") { m.refuse() }
    }

    // ---- D2 the optical exchange, mutual and on the selfie cameras -----

    /**
     * **Turn the phone to face the counterparty** [author, 2026-09-29].
     * This is the moment the device stops being its user's: from here it
     * shows its code to the other person and reads theirs, and takes no
     * input until the capture is done.
     */
    private fun optical(m: Meet) {
        para("↻  TURN YOUR PHONE AROUND so the screen faces ${m.counterpartyName}, and let them do the same. Each phone reads the other's code with its SELFIE camera.")
        para("Two codes cross, in order: each phone's contribution, then the meeting id both compute from the pair. If the two ids differ, something is between you and the meeting stops — that check is the whole of what looking at each other's screen buys.")
        if (!m.begun()) {
            para("Opening the ceremony…")
            return
        }
        // the object in parts, crossed in step with the other side
        // (`OpticalExchange`): the exchange outlives the redraws, and a new
        // one opens for the transcript once the contribution is taken
        val transcript = Kernel.opticalTaken()
        val which = if (transcript) OpticalExchange.TRANSCRIPT else OpticalExchange.CONTRIBUTION
        // **The contribution goes in colour until a camera says it cannot**
        // ([Polychrome]): three parts a frame at eleven bytes each, which
        // is the smallest symbol there is and the longest read. The
        // transcript is one part and stays monochrome, which is also what
        // keeps `which` out of the compressed header.
        val colour = !transcript && !m.colourRefused
        val chunk = if (colour) OpticalExchange.CHUNK_POLY else OpticalExchange.CHUNK
        val x = m.exchange?.takeIf { it.which == which && it.chunk == chunk } ?: run {
            val code = Kernel.optical()
            if (code == null) {
                para("The code needs an open ceremony, which this device has lost.")
                return
            }
            if (colour) m.colourSince = System.currentTimeMillis()
            OpticalExchange(which, code, chunk).also { m.exchange = it }
        }
        val name = if (transcript) "transcript" else "contribution"
        if (colour) {
            // **the frame that decides it carries full headers**, so each
            // channel is a self-standing part and one of them brings the
            // count the compressed headers leave out; every frame after it
            // is compressed
            val confirmed = x.theirReceived() >= Polychrome.CHANNELS
            val began = m.colourSince
            if (!confirmed && began != null && System.currentTimeMillis() - began > Meet.COLOUR_PROBE_MS) {
                // their camera has had several frames and holds none of
                // this side's parts: a colour frame decodes to nothing at
                // all where the channels do not separate, so this is what
                // that failure looks like from here
                Diag.event("optical.colour", "state" to "refused", "they_hold" to x.theirReceived())
                m.colourRefused = true
                m.exchange = null
                m.colourSince = null
                // **the camera goes with it**: its colour sink outlives a
                // redraw, and three decodes a frame is a cost the
                // monochrome path should not keep paying once colour has
                // been given up on
                camera?.close()
                camera = null
                redraw()
                return
            }
            qrColour(x.colourFrames(confirmed), "$name ${x.showing() + 1}/${x.count}")
            para(x.status())
        } else {
            qr(x.frame(), "$name ${x.showing() + 1}/${x.count}")
            para(x.status())
        }
        if (x.done()) {
            // this side is through; the last code stays up for the other
            // side's camera until the kernel moves on (the transcript's
            // exchange, or their intent arriving over the bearer)
            para(if (transcript) "Both codes agree. Waiting for the other phone to catch up before the exchange moves on." else "Their contribution is in; the meeting id follows.")
            return
        }
        aim()
        scan(QrCamera.Facing.SELFIE, name, continuous = true, channels = colour) { bytes, ch ->
            runOnUiThread {
                if (m.step() != Meet.Step.OPTICAL || m.exchange !== x) return@runOnUiThread
                when (x.take(bytes, ch)) {
                    OpticalExchange.Took.NOT_OURS -> return@runOnUiThread
                    OpticalExchange.Took.MALFORMED -> { m.stop("a code of the exchange did not read as one"); return@runOnUiThread }
                    else -> {}
                }
                Diag.event("optical.exchange", "which" to name, "showing" to x.showing(), "received" to x.received(), "theirs" to x.theirCount(), "they_hold" to x.theirReceived())
                if (x.done()) {
                    // both hold everything: the object goes to the kernel;
                    // the exchange stays so its last code stays up, and the
                    // next redraw opens the transcript's or the kernel has
                    // moved on
                    camera?.close()
                    val why = Kernel.takeOptical(x.theirs()!!)
                    if (why != null) { m.stop(why); return@runOnUiThread }
                }
                redraw()
            }
        }
    }

    // ---- D3 proximity: the tap, and the optical pass D2 already made ---

    /**
     * The kernel runs the channel ladder (design §7.6.3) off the UI thread
     * and the screen follows. **The phone still faces the counterparty**,
     * so there is nothing to tap here: the NFC tap is two phones touching,
     * and the optical channel already passed when the two screens agreed at
     * D2. The strongest that passed is what the record carries.
     */
    private fun proximity(m: Meet) {
        para("Ranging with ${m.counterpartyName}. Hold the phones together for the tap; the screen-to-screen check at the last step is itself the weakest channel, so this cannot come back with nothing.")
        para("UWB is not among the channels this build can run, so the strongest here is a tap. The record carries the strongest that passed and no more — a weaker channel is never shown as a stronger one.")
        once(m, "proximity") {
            Kernel.runProximity(m)
        }
    }

    // ---- D4 capture: the counterparty's face ---------------------------

    /**
     * The guided capture runs on the kernel's thread, one frame per prompt,
     * on the selfie camera the phone is already pointing at the
     * counterparty. The disclosures were read at D1.5 and the device faces
     * away, so this shows a progress line and takes nothing.
     */
    private fun capture(m: Meet) {
        para("Capturing ${m.counterpartyName}: a few frames over a few seconds, with spoken or toned prompts for them. Nothing is shown to you and nothing is asked — you read what this holds and who may see it before the phone turned around.")
        if (!held(Manifest.permission.CAMERA)) {
            para("The capture needs the camera, which is not yet allowed.")
            button("Allow the camera") { ask(2, Manifest.permission.CAMERA) }
            return
        }
        once(m, "capture") {
            // the conversation on the courier opens from the kernel as
            // the captures seal; nothing here starts it
            Kernel.runCapture(m)
        }
    }

    /**
     * Run `work` once as the flow enters a hands-off step, off the UI
     * thread, and never again on a redraw. A recreated screen rejoins a
     * step already running rather than starting it twice.
     */
    private fun once(m: Meet, tag: String, work: () -> Unit) {
        para("Working…")
        if (started.add("${m.counterpartyKey}:$tag")) {
            Thread { work() }.start()
        }
    }

    // ---- D5 verifiers --------------------------------------------------

    private fun verifiers(m: Meet) {
        para("Your device picks ${m.counterpartyName}'s verifiers from the records they handed over, preferring people you have met. The choice is computed, not offered: there is nothing here to pick.")
        if (!m.selectionRun()) {
            // the selection is the kernel's, asked for once as the flow
            // enters this step, and never from inside a draw
            para("Selecting…")
            return
        }
        val chosen = m.chosen()
        heading("Selected")
        if (chosen.isEmpty()) {
            para("• None required. ${m.counterpartyName} handed over no records, so there is no pool to draw from and none is owed; a meeting with fewer verifiers is thinner, not malformed.")
        } else {
            chosen.forEach { c ->
                val answer = when (c.verdict) {
                    null -> "awaiting an answer"
                    Meet.Verdict.MATCH -> "answered: a match"
                    Meet.Verdict.NO_MATCH -> "answered: no match"
                    Meet.Verdict.INCONCLUSIVE -> "answered: inconclusive"
                    Meet.Verdict.UNAVAILABLE -> "unavailable; silence counts for nothing either way"
                }
                para("• ${c.key.take(16)}…  ${basisWords(c.basis)}; $answer")
            }
            para("Each query goes to ${m.counterpartyName}'s device for consent on the same end-to-end path the rest of this conversation uses. Consented, the kernel carries it to its verifier and the answer lands above.")
        }
        val mine = m.queriesAboutMe()
        if (mine.isNotEmpty()) {
            heading("Queries about you")
            para("${m.counterpartyName} asked these verifiers about you. Your device consented on each: consent is bound to the ceremony you are standing in, so it is given without stopping to ask, and you are told instead.")
            mine.forEach { para("• $it…") }
        }
        witnesses(m)
        heading("The conversation")
        val p = m.progress()
        when {
            !m.opened() -> para("Opening: the nominees are being asked to witness, and the back-pointers are going out.")
            p == null -> para("Waiting on the kernel's first word of where it stands.")
            p.queriesOutstanding > 0 -> {
                val left = (m.patienceLeftMs() ?: 0L) / 1000
                para("${p.queriesOutstanding} of the queries have no answer yet. A verifier that never answers does not appear in the record; this device goes on without it in $left s, or now if you say so.")
                button("Go on without them") { m.goOn() }
            }
            p.proposer && m.noWitness() ->
                para("No nominee has agreed to attest, and the body cannot be proposed without one. Your device keeps asking as the answers land. Wait for one, or stop and leave this meeting unrecorded.")
            p.proposer && m.waitingOn() != null -> para("Not yet proposed: waiting on ${m.waitingOn()}.")
            p.proposer -> para("Proposing the body to every signer.")
            else -> para("Your gathered responses are with ${m.counterpartyName}, whose device proposes the body. It is shown here when it arrives, reviewed and signed by this device as it does.")
        }
        button("Stop") { m.stop("you stopped at the verifiers") }
    }

    /** Every nominee, whose it was and what it answered: a witness that
     *  declined is shown as declined, not dropped, since the record will
     *  show the slot it leaves (design §7.1). */
    private fun witnesses(m: Meet) {
        heading("Witnesses")
        val w = m.witnesses()
        if (w.isEmpty()) {
            para("• None nominated. With no horizon to draw from, neither side had anyone to ask; the record carries that it had none.")
            return
        }
        w.forEach { n ->
            val by = if (n.mine) "nominated by you" else "nominated by ${m.counterpartyName}"
            val answer = when (n.answer) {
                null -> "not yet answered"
                Meet.Answer.ATTESTS -> "will attest"
                Meet.Answer.DECLINED -> "declined"
            }
            para("• ${n.key.take(16)}…  $by; $answer")
        }
    }

    /** `wire-format.md` §5.5's basis, in words, and each says whose claim
     *  it is: nobody audits a selector's tier. */
    private fun basisWords(b: Meet.Basis): String = when (b) {
        Meet.Basis.MET -> "you have met them"
        Meet.Basis.IN_HORIZON -> "in your horizon"
        Meet.Basis.REACHABLE -> "one edge beyond it"
        Meet.Basis.DISCRETIONARY -> "a stranger, taken at your discretion"
    }

    // ---- D6 review and sign --------------------------------------------

    /**
     * **The body, as signed.** The kernel reviews the body against what
     * this device holds and signs it as it arrives, or refuses it with a
     * code (`wire-format.md` §7.10.2), so by this screen the signature has
     * gone: what is owed here is to show what was signed, and the
     * weaknesses UX-003 names, and to let the person stop watching.
     */
    private fun review(m: Meet) {
        val p = m.progress()
        val proposer = p?.proposer == true
        para(
            if (proposer) {
                "Your device proposed the body and signed it. Every other signer is shown it now and answers with a signature or a refusal."
            } else {
                "${m.counterpartyName}'s device proposed the body. Yours checked it against what it holds, the root, the back-pointers and the disclosures, and signed it; a body that did not check would have been refused with its code and this meeting stopped."
            },
        )
        heading("Signers")
        para("• You")
        para("• ${m.counterpartyName}")
        m.witnesses().filter { it.answer == Meet.Answer.ATTESTS }.forEach { n ->
            para("• ${n.key.take(16)}…  witness, nominated by ${if (n.mine) "you" else m.counterpartyName}")
        }
        heading("Signatures")
        if (proposer && p != null) {
            para("${p.signed.size} of the signers have signed so far, this device among them. The record is finalized here when every one has, and goes to each of them.")
            p.refused.forEach { para("• ${it.take(16)}…  refused to sign") }
        } else {
            para("The record arrives from ${m.counterpartyName} once every signer has signed, is checked against this body, and is held.")
        }
        val warnings = m.presign()
        if (warnings.isEmpty()) {
            heading("Nothing to flag")
            para("Every check this screen makes came back unremarkable. That is not a guarantee about the person in front of you; it is the absence of the specific weaknesses listed at D6.")
        } else {
            heading("What is thin about this meeting")
            warnings.forEach { para("• ${it.text}") }
            // UX-003's whole point, said where the person reads it
            para("None of the above makes the record broken. Each is something a reader of the record can see for themselves, and a meeting that says less is still a meeting that happened.")
        }
        para("Your signature has gone and is not withdrawn from here. Stopping leaves the record to finalize, or not, without this screen.")
        button("Stop") { m.stop("you stopped at review") }
    }

    // ---- D7 after ------------------------------------------------------

    private fun done(m: Meet) {
        para("The record: ${m.recordTxid()}")
        when {
            // PRD-05: two people who each chose to be the patron have not
            // hit a protocol failure, and the screen must not say they have
            m.opposedAdoptions() && m.direction() == null -> {
                heading("You both offered to be the patron")
                para("Each of you chose a direction before seeing the other's, so this is two intentions rather than a fault — nothing malformed has happened, and the meeting itself stands whichever way this goes.")
                para("Choose a direction between you, or leave the authority question alone.")
                button("I will be the patron") { m.chooseDirection(Meet.Adopt.THEM_UNDER_ME) }
                button("${m.counterpartyName} will be the patron") {
                    m.chooseDirection(Meet.Adopt.ME_UNDER_THEM)
                }
                button("Neither — just the meeting") { m.chooseDirection(Meet.Adopt.NONE) }
                return
            }
            m.direction() == Meet.Adopt.NONE ->
                para("You left the authority question alone. The meeting stands on its own, which costs neither of you anything you had.")
            m.direction() != null || m.adopt != Meet.Adopt.NONE ->
                para("The adoption would now be proposed and taken in the direction settled here.")
        }
        button("Done") { Kernel.stopMeet("done"); finish() }
    }

    private fun stopped(m: Meet) {
        para("Stopped: ${m.stopReason()}")
        button("Back") { Kernel.stopMeet("dismissed"); finish() }
    }

    // ---- view helpers --------------------------------------------------

    private fun title(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 24f
            typeface = Typeface.DEFAULT_BOLD
            setPadding(0, 0, 0, 24)
        },
    )

    private fun heading(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 16f
            typeface = Typeface.DEFAULT_BOLD
            setPadding(0, 20, 0, 8)
        },
    )

    private fun para(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 15f
            setPadding(0, 8, 0, 8)
        },
    )

    private fun button(label: String, onClick: () -> Unit) = body.addView(
        Button(this).apply {
            text = label
            setPadding(0, 16, 0, 16)
            setOnClickListener { onClick() }
        },
    )

    private fun logLines(m: Meet) {
        m.log().forEach { line ->
            body.addView(
                TextView(this).apply {
                    text = line
                    textSize = 12f
                    typeface = Typeface.MONOSPACE
                    setTextColor(Color.GRAY)
                    setPadding(0, 2, 0, 2)
                },
            )
        }
        scroll.post { scroll.fullScroll(ScrollView.FOCUS_DOWN) }
    }
}
