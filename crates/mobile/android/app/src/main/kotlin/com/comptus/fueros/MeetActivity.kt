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
 * The flow, its front-loaded brief, its hands-off phase and its
 * review-before-sign gate are enforced by [Meet]. **D1, D2 and D4 run on
 * the hardware** (D3, the tap, is retired [author, 2026-10-24] and its
 * number is not reused): the invitation and anchor codes on the rear and
 * selfie cameras, the intent over a Bluetooth LE bearer and the guided
 * capture on the selfie camera. **D5 through D7 run on the kernel's
 * courier**: the request to witness, the queries and their consent, the
 * gathered responses, the body and the record cross the end-to-end path
 * (`wire-format.md` §7.10.1), the kernel reviews and signs as the body
 * arrives, and the flow moves on what the kernel reports. The screens from
 * D5 on show and let the person stop; nothing there is a control over the
 * protocol.
 */
class MeetActivity : Activity() {

    private lateinit var body: LinearLayout
    private lateinit var scroll: ScrollView
    private var camera: QrCamera? = null

    /** The symbol's geometry as last drawn, so the tracking rows under it
     *  match its module size exactly ([trackingUnder]). */
    private var lastModules = 0
    private var lastScale = 0

    /**
     * **The two images a turn of the window repaints** ([repaint]), rather
     * than rebuilding the page: a rebuild begins with `removeAllViews()`,
     * and at ten turns a second a button pressed on one instance is
     * released on another and does nothing — which is how the *Allow the
     * camera* button came to be unusable [author, 2026-10-07]. Nothing but
     * these two images changes from one turn to the next.
     */
    private var codeView: ImageView? = null
    private var trackView: ImageView? = null

    /** Whether this redraw put the optical package on screen, which
     *  decides what else is drawn and where the page is scrolled to. */
    private var optical = false
    /** The step last drawn, so a step is cued once, on arrival. */
    private var drawnStep: Meet.Step? = null

    private val turning = android.os.Handler(android.os.Looper.getMainLooper())
    private var turner: Runnable? = null

    /**
     * **What advances the optical window.** The exchange rotates
     * (`OpticalExchange.showing`), so something has to turn it, and the
     * screen is the only thing that knows when a frame has been up long
     * enough (`Meet.TURN_MS`). Posted only while a code is on screen and
     * cancelled the moment it is not: a timer redrawing a screen that has
     * moved on is how a camera ends up held behind a dead ceremony.
     */
    private fun turnWhileShowing(x: OpticalExchange) {
        // **started once and left alone.** Cancelling and re-posting on
        // every redraw would let a steady stream of incoming reads reset
        // the clock for ever, and the window would never turn — the reads
        // are of *their* parts and the rotation is of this side's, so the
        // two must not be coupled.
        if (turner != null) return
        val r = object : Runnable {
            override fun run() {
                val m = Kernel.meet()
                val live = m != null && m.step() == Meet.Step.OPTICAL && m.exchange === x
                if (!live) {
                    turner = null
                    return
                }
                // **a counterparty that has gone is not waited on for
                // ever** ([Meet.opticalStale]). The window turning is the
                // only thing still running at this step, so it is the only
                // place that can notice.
                if (m.opticalStale()) {
                    turner = null
                    m.stop("nothing has crossed for ${Meet.STALE_MS / 1000} s; the other phone may have stopped")
                    return
                }
                x.turn()
                // reschedule before drawing: redraw is what calls back in
                // here, and this runnable is what owns the cadence
                turning.postDelayed(this, Meet.TURN_MS)
                // **repaint, do not rebuild** ([repaint]): the page's other
                // views — a permission button among them — must survive a
                // press that spans two turns
                if (!repaint(x)) redraw()
            }
        }
        turner = r
        turning.postDelayed(r, Meet.TURN_MS)
    }

    private fun stopTurning() {
        turner?.let { turning.removeCallbacks(it) }
        turner = null
    }
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
        Kernel.meet()?.bind(sink) ?: redraw()
    }

    override fun onStop() {
        Consent.release(this)
        Kernel.meet()?.unbind(sink)
        // **leaving before anything began is changing one's mind**: a
        // meeting still at the choice or the brief has nothing in the
        // kernel yet, and keeping it would hand the same choice back to
        // the next opening of this screen [author, 2026-10-11]
        if (isFinishing) {
            Kernel.meet()?.let { m ->
                if (m.step() == Meet.Step.INTENT || m.step() == Meet.Step.BRIEF) Kernel.stopMeet("dismissed")
            }
        }
        // the camera goes back the moment this screen stops: holding one
        // behind a screen nobody is looking at is a camera nobody consented
        // to
        camera?.close()
        camera = null
        // and the window stops turning with it: a timer redrawing a screen
        // nobody is looking at is the same kind of leak
        stopTurning()
        super.onStop()
    }

    private fun redraw() {
        body.removeAllViews()
        codeView = null
        trackView = null
        previewHost.visibility = android.view.View.GONE
        optical = false
        val m = Kernel.meet()
        if (m == null) {
            entry()
            return
        }
        // **nothing on this screen but the optical channel and the
        // viewfinder** [author, 2026-10-07]. Set before a single view is
        // added, because the title and the instructions come first and
        // between them cost about five hundred pixels — which is what
        // pushed the tracking rows off the bottom.
        optical = m.step() == Meet.Step.OPTICAL
        title(
            when (m.step()) {
                Meet.Step.INTENT -> "Meet ${m.counterpartyName}"
                Meet.Step.BRIEF -> "Before you begin"
                Meet.Step.OPTICAL -> "Exchanging (1/2)"
                Meet.Step.WINDOW -> "In the window (2/2)"
                Meet.Step.REVIEW -> "Review and sign"
                Meet.Step.DONE -> "Done"
                Meet.Step.STOPPED -> "Stopped"
            },
        )
        // **the scope question comes first** [author, 2026-10-15]: for the
        // initiator between the choice of transaction and the invitation,
        // for the party that read it between the scan and the acceptance
        val choice = if (m.step() == Meet.Step.INTENT || m.step() == Meet.Step.BRIEF) {
            m.pendingChoice(Kernel.scopes(), Kernel.scopesRooted())
        } else {
            null
        }
        when {
            choice != null -> choice(m, choice)
            else -> when (m.step()) {
                Meet.Step.INTENT -> intent(m)
                Meet.Step.BRIEF -> brief(m)
                Meet.Step.OPTICAL -> optical(m)
                Meet.Step.WINDOW -> window(m)
                Meet.Step.REVIEW -> review(m)
                Meet.Step.DONE -> done(m)
                Meet.Step.STOPPED -> stopped(m)
            }
        }
        // **the switch to capture is cued**, once, as it happens: with the
        // phones held up for the codes, nobody is looking at either screen
        // [author, 2026-10-11]
        if (m.step() == Meet.Step.WINDOW && drawnStep != Meet.Step.WINDOW) cueCapture()
        drawnStep = m.step()
        atTop()
    }

    /**
     * A short buzz and a beep: the codes are read, the camera is next. Both
     * are best effort — a phone with no vibrator or a muted tone channel
     * still shows the step — and neither carries anything about the
     * ceremony beyond the fact that it moved.
     */
    private fun cueCapture() {
        runCatching {
            val v = getSystemService(android.content.Context.VIBRATOR_SERVICE) as? android.os.Vibrator
            v?.vibrate(android.os.VibrationEffect.createOneShot(150, android.os.VibrationEffect.DEFAULT_AMPLITUDE))
        }
        runCatching {
            val tone = android.media.ToneGenerator(android.media.AudioManager.STREAM_NOTIFICATION, 80)
            tone.startTone(android.media.ToneGenerator.TONE_PROP_ACK, 200)
            body.postDelayed({ runCatching { tone.release() } }, 400)
        }
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
            // a refusal the system will not ask about again is the one
            // that matters ([settled])
            settle(permissions[i], refused = !granted)
            Diag.event(
                "permission",
                "which" to short(permissions[i]),
                "state" to if (granted) "granted" else "denied",
                "again" to (granted || shouldShowRequestPermissionRationale(permissions[i])),
            )
        }
        redraw()
    }

    /**
     * **Permissions asked for and refused**, kept across restarts. Android
     * shows its dialogue again after one refusal and not after two, and
     * `shouldShowRequestPermissionRationale` is false in *both* the
     * never-asked and the asked-twice cases — so whether asking would show
     * anything cannot be read before asking, and a refusal that was not
     * remembered left the exchange screen a dead end with a button that
     * did nothing [author, 2026-10-07].
     */
    private val settled: android.content.SharedPreferences
        get() = getSharedPreferences("permissions", MODE_PRIVATE)

    private fun settle(permission: String, refused: Boolean) =
        settled.edit().putBoolean(permission, refused).apply()

    /** Whether asking again would show the person anything. */
    private fun willAsk(permission: String): Boolean =
        !settled.getBoolean(permission, false) || shouldShowRequestPermissionRationale(permission)

    /** The app's own settings page, which is where a refusal the system
     *  will not re-ask about has to be undone. */
    private fun openSettings() {
        Diag.event("permission", "which" to "camera", "state" to "settings")
        runCatching {
            startActivity(
                android.content.Intent(
                    android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                    android.net.Uri.fromParts("package", packageName, null),
                ),
            )
        }.onFailure { para("This device would not open the settings page.") }
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
        // **A meeting is with a stranger** [author, 2026-10-07]: the
        // invitation code is the first thing either device knows of the
        // other, so nothing here needs to know who that is. One side shows
        // a code naming itself and the other scans it; the optical
        // contribution then carries the key material the kernel pins
        // (`wire-format.md` §14.3.1).
        //
        // D1a, the initiator's dialogue, as the author specified it: a
        // regular meeting is the DEFAULT, with the backup checkbox beside
        // it, and patronage a separate option that then asks the direction.
        para("A meeting establishes a presence record with the other person, in person. What this meeting is for is chosen now and cannot change once it begins.")
        heading("Regular meeting")
        // **the peering is the checkbox on the regular meeting**, as the
        // author's dialogue has it [author, 2026-10-19]. It read "Ask this
        // person to backup my user data" and set a flag that reached the
        // brief and nothing else; the backup is what a peering entitles
        // the two of you to (design §3.4), so the box asks for the
        // peering and the backup follows from it.
        val peer = CheckBox(this).apply {
            text = "Also peer with this person"
            setTextColor(Color.DKGRAY)
        }
        body.addView(peer)
        para("A peering is an edge between the two of you where neither is over the other, priced in this meeting (design §6.3). It is what lets either of you hold an encrypted backup of the other's contacts and records.")
        button("Show my code") {
            start(Meet.Kind(Meet.Adopt.NONE, peer.isChecked), Meet.Role.INITIATOR)
        }
        heading("Patronage")
        para("An adoption moves authority. What it costs is listed before either of you agrees.")
        // which scope, either way, is the next screen's question
        // ([choice]), asked only of a device with something to choose
        button("I will be the Patron") {
            start(Meet.Kind(Meet.Adopt.THEM_UNDER_ME), Meet.Role.INITIATOR)
        }
        button("I will be the Client") {
            start(Meet.Kind(Meet.Adopt.ME_UNDER_THEM), Meet.Role.INITIATOR)
        }
        heading("Or join theirs")
        para("The other person has shown you a code. Their choice of transaction is in it, and the next screen is where you accept or refuse it.")
        button("Scan a QR code") { start(Meet.Kind(), Meet.Role.RESPONDER) }
        // **the responder's own D1a** (design §7.1; PRD-05): each side chooses
        // a direction before seeing the other's, which is how two people
        // both offer to be the patron — an ordinary thing and not a fault.
        // Until here the shell let only the inviter choose, so the opposed
        // case could not arise on the bench [bench, 2026-10-24]. Developer
        // wording until the UX pass.
        para("Or scan with a choice of your own, made before you see theirs:")
        button("Scan — I will be the Patron") {
            start(Meet.Kind(Meet.Adopt.THEM_UNDER_ME), Meet.Role.RESPONDER)
        }
        button("Scan — I will be the Client") {
            start(Meet.Kind(Meet.Adopt.ME_UNDER_THEM), Meet.Role.RESPONDER)
        }
    }

    /**
     * **A choice made in error is undone here**, not by relaunching the
     * app [author, 2026-10-11]: until the brief is accepted nothing has
     * begun in the kernel, so the meeting is dropped and the entry screen
     * offers the choice again.
     */
    private fun changeMind() {
        button("Start over — choose again") {
            Kernel.stopMeet("dismissed")
            redraw()
        }
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
                changeMind()
            }
            Meet.Role.RESPONDER -> {
                para("Point the back of your phone at ${m.counterpartyName}'s screen. Their code carries who they are and the kind of meeting they chose; the next screen is where you accept or refuse it.")
                changeMind()
                aim("The band at the top is the middle of what the rear camera sees: put their code in it.")
                scan(QrCamera.Facing.REAR, "bootstrap") { bytes ->
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

    /** A code on the screen, as large as the layout allows. The module's
     *  physical size is its width as a fraction of the code's own, never a
     *  pixel count, so the diagnostics carry the module's millimetres and
     *  the symbol's modules; what distance that reads at is not predicted
     *  from it (`OpticalExchange.CHUNK`). */
    private fun qr(bytes: ByteArray, which: String, trackingParts: Int = 0) {
        val m = Optical.matrix(bytes)
        // **As wide as the content area** [author, 2026-10-04]: the
        // module's physical size is this width over the symbol's modules.
        // The area is the body's own width once laid out, never the
        // screen's: a code wider than its parent is clipped at the right,
        // quiet zone and modules, and reads nowhere
        val avail = if (body.width > 0) body.width else resources.displayMetrics.widthPixels - 96 - 48
        // **and it has to fit the page's height too**: the code and its
        // tracking rows are one instrument, visible to the other person's
        // camera at once [author, 2026-10-07], so the module is bounded by
        // what is left vertically as well as by the width
        val rowModules = if (trackingParts > 0) Tracking.rows(trackingParts, m.width) * Tracking.SCALE else 0
        val availH = heightForCode()
        val byWidth = (avail - 8) / m.width
        val byHeight = if (availH > 0) availH / (m.width + rowModules) else byWidth
        val scale = maxOf(1, minOf(byWidth, byHeight))
        // the module's physical size, from the display's own dpi: a fact
        // about the code, reported so a run on another phone compares
        // [author, 2026-10-06]. No read distance is predicted from it
        val dpi = resources.displayMetrics.xdpi
        val moduleMm = if (dpi > 0f) scale / dpi * 25.4f else 0f
        Diag.event(
            "qr.shown", "which" to which, "bytes" to bytes.size, "modules" to m.width,
            "scale" to scale, "module_mm" to String.format("%.2f", moduleMm),
        )
        lastModules = m.width
        lastScale = scale
        val w = m.width * scale
        val px = IntArray(w * w)
        for (y in 0 until w) {
            for (x in 0 until w) {
                px[y * w + x] = if (m.get(x / scale, y / scale)) Color.BLACK else Color.WHITE
            }
        }
        show(px, w)
    }

    /**
     * **The tracking rows, flush under the symbol** ([Tracking]): which of
     * the counterparty's parts are held here, one module each, so they can
     * rotate through what is actually still owed rather than through what
     * a contiguous count cannot describe.
     *
     * Drawn as a second image rather than inside the symbol's matrix: the
     * symbol has to stay a well-formed code for the decoder, and the rows
     * carry no error correction of their own.
     */
    private fun trackingUnder(x: OpticalExchange, modules: Int, scale: Int) {
        val bits = x.tracking()
        if (bits.isEmpty()) return
        val (px, w) = Tracking.pixels(bits, bits.size, modules, scale)
        val h = Tracking.rows(bits.size, modules) * Tracking.SCALE * scale
        if (h <= 0) return
        Diag.event(
            "qr.tracking", "parts" to bits.size, "held" to bits.count { it },
            "rows" to Tracking.rows(bits.size, modules), "across" to Tracking.across(modules),
            "module_px" to Tracking.SCALE * scale,
        )
        val v = ImageView(this).apply {
            setImageBitmap(Bitmap.createBitmap(px, w, h, Bitmap.Config.ARGB_8888))
            layoutParams = LinearLayout.LayoutParams(w, h)
        }
        trackView = v
        body.addView(v)
    }

    /**
     * **What vertical room the code and its rows have**: the scrolling
     * page's own height, less the aiming band when that is up. Measured
     * where the views have been laid out, estimated from the display
     * where they have not, since the first draw precedes either having a
     * height.
     */
    private fun heightForCode(): Int {
        val page = if (scroll.height > 0) scroll.height else resources.displayMetrics.heightPixels - 420
        val band = when {
            previewHost.visibility != android.view.View.VISIBLE -> 0
            previewHost.height > 0 -> previewHost.height
            else -> resources.displayMetrics.heightPixels / 8
        }
        return page - band - 48
    }

    /** Pixels into the view, which the two renderers share. */
    private fun show(px: IntArray, w: Int) {
        val v = ImageView(this).apply {
            setImageBitmap(Bitmap.createBitmap(px, w, w, Bitmap.Config.ARGB_8888))
            layoutParams = LinearLayout.LayoutParams(w, w).apply { topMargin = 24 }
        }
        codeView = v
        body.addView(v)
    }

    /**
     * **One turn of the window, repainting and nothing else** ([codeView]).
     * False where the views are not up, which is the caller's cue to go
     * the long way round.
     */
    private fun repaint(x: OpticalExchange): Boolean {
        val code = codeView ?: return false
        val m = runCatching { Optical.matrix(x.frame()) }.getOrNull() ?: return false
        if (m.width != lastModules || lastScale <= 0) return false
        val scale = lastScale
        val w = m.width * scale
        val px = IntArray(w * w)
        for (y in 0 until w) {
            for (cx in 0 until w) {
                px[y * w + cx] = if (m.get(cx / scale, y / scale)) Color.BLACK else Color.WHITE
            }
        }
        code.setImageBitmap(Bitmap.createBitmap(px, w, w, Bitmap.Config.ARGB_8888))
        val bits = x.tracking()
        val track = trackView
        if (track != null && bits.isNotEmpty()) {
            val (tpx, tw) = Tracking.pixels(bits, bits.size, m.width, scale)
            val th = Tracking.rows(bits.size, m.width) * Tracking.SCALE * scale
            if (th > 0) track.setImageBitmap(Bitmap.createBitmap(tpx, tw, th, Bitmap.Config.ARGB_8888))
        }
        return true
    }

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

    /**
     * Read a code with the named camera. The permission is asked for here,
     * and a camera that fails after it was asked for stops the ceremony
     * with the reason rather than silently.
     */
    private fun scan(
        facing: QrCamera.Facing,
        which: String,
        continuous: Boolean = false,
        found: (ByteArray) -> Unit,
    ) {
        if (!held(Manifest.permission.CAMERA)) {
            // the one thing that still speaks over the code: without it
            // the screen is a dead end
            if (willAsk(Manifest.permission.CAMERA)) {
                paraAlways("This step needs the camera. Nothing is read until you allow it.")
                buttonAlways("Allow the camera") { ask(1, Manifest.permission.CAMERA) }
            } else {
                // **the system will not ask again**, so offering a button
                // that calls `requestPermissions` would be offering
                // nothing. Its settings page is the only place left where
                // the answer can be changed.
                paraAlways("This step needs the camera, and Android will not ask again after a refusal. Turn it on in this app's settings and come back — the meeting is still here.")
                buttonAlways("Open app settings") { openSettings() }
            }
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
        cam.readOne(facing, surface, which, onFailed, continuous, found = found)?.let { why -> para(why) }
    }

    // ---- the scope question, one screen in two versions ---------------

    /**
     * **"Adopt into which scope?" or "Submit as which scope?"** [author,
     * 2026-10-15]: the would-be patron chooses which of its subnets the
     * other joins, a new one under its own key among them; the would-be
     * subordinate chooses which scope it roots it brings along, or none.
     * Shown before the invitation for the initiator and before the
     * acceptance for the party that read it; a device with nothing to
     * choose never sees it.
     */
    private fun choice(m: Meet, c: Meet.Choice) {
        when (c) {
            Meet.Choice.ADOPT_INTO -> {
                heading("Adopt into which scope?")
                para("${m.counterpartyName} will sit one hop below you. Which of your subnets do they join?")
                (Kernel.scopes() + Meet.Scope.OWN).forEach { s ->
                    button(s.label()) { m.chooseScope(s) }
                }
            }
            Meet.Choice.SUBMIT_AS -> {
                heading("Submit as which scope?")
                para("${m.counterpartyName} will be your patron. Do you bring a scope you root with you — everyone in it coming to sit in their subnet — or join as an individual, your scopes standing as they are?")
                Kernel.scopesRooted().forEach { s ->
                    button("Bring ${s.label()}") { m.chooseCarrying(s) }
                }
                button("Join as an individual") { m.chooseCarrying(Meet.Scope.INDIVIDUAL) }
            }
        }
        changeMind()
    }

    // ---- D1.5 the brief: everything front-loaded -----------------------

    private fun brief(m: Meet) {
        para("This is what is about to happen. Both of you are being shown it, and both of you answer.")
        // **what the two of them are about to do**, which for a responder
        // is what the initiator's invitation asked for and not what this
        // device chose [bench, 2026-10-15]
        val into = m.subnetLabel()?.let { " — into $it" } ?: ""
        heading(
            when (m.settled()) {
                null -> "One of you as the other's patron"
                Meet.Adopt.NONE -> "A regular meeting with ${m.counterpartyName}"
                Meet.Adopt.ME_UNDER_THEM -> "${m.counterpartyName} as your patron$into"
                Meet.Adopt.THEM_UNDER_ME -> "You as ${m.counterpartyName}'s patron$into"
            },
        )
        // every item the model says this ceremony earns, and no reassurance
        // it has not earned
        m.brief().forEach { para("• ${it.text}") }

        heading("What may be weak in this meeting")
        para("• Nominated witnesses: none, until your horizon can offer them. The record carries that it had none.")
        para("• Verifiers are weighed, not required; a thin meeting is honest, not malformed.")
        if (bluetooth.any { !held(it) }) {
            // the bearer that carries the intent is Bluetooth LE; without
            // the permissions the exchange stops at the radio, said here
            // where the person can still act rather than caught there
            para("The intent crosses over Bluetooth, which this app is not yet allowed to use. Without it the meeting will stop at the exchange.")
            button("Allow Bluetooth") { ask(3, *bluetooth) }
        }
        button("Accept — begin") { m.accept() }
        button("Refuse") { m.refuse() }
        changeMind()
    }

    // ---- D2 the optical exchange, mutual and on the selfie cameras -----

    /**
     * **Turn the phone to face the counterparty** [author, 2026-09-29].
     * This is the moment the device stops being its user's: from here it
     * shows its code to the other person and reads theirs, and takes no
     * input until the capture is done.
     */
    private fun optical(m: Meet) {
        if (!m.begun()) return
        // the object in parts (`OpticalExchange`): the exchange outlives
        // the redraws, and a new one opens for the transcript once the
        // contribution is taken
        val transcript = Kernel.opticalTaken()
        val which = if (transcript) OpticalExchange.TRANSCRIPT else OpticalExchange.CONTRIBUTION
        val x = m.exchange?.takeIf { it.which == which } ?: run {
            val code = Kernel.optical() ?: return m.stop("the kernel would not give the code to show")
            OpticalExchange(which, code).also { m.exchange = it }
        }
        val name = if (transcript) "transcript" else "contribution"
        qr(x.frame(), "$name ${x.showing() + 1}/${x.count}", x.theirCount())
        trackingUnder(x, lastModules, lastScale)
        // the window turns on the screen's own clock, not on a read
        // landing ([turnWhileShowing]) — and not at all while this side
        // cannot read, since the screen should sit still under whatever
        // the person is answering
        if (!x.done() && held(Manifest.permission.CAMERA)) turnWhileShowing(x) else stopTurning()
        // this side through: the last code stays up for the other side's
        // camera until the kernel moves on
        if (x.done()) return
        aim()
        // **the sampler learns their size as soon as a code of theirs
        // reads** ([Tracking]): their part count is in that first header,
        // and their symbol is as wide as this side's, both being this
        // build at the same chunk. Until then there is nothing to sample.
        val theirs = x.theirCount()
        if (theirs > 0 && lastModules > 0) {
            camera?.trackingOf(theirs, lastModules) { bits, under ->
                runOnUiThread {
                    val m2 = Kernel.meet() ?: return@runOnUiThread
                    if (m2.step() != Meet.Step.OPTICAL || m2.exchange !== x) return@runOnUiThread
                    // rows under this side's own code, reflected, are this
                    // side's own holdings and say nothing about theirs
                    if (x.mine(under)) return@runOnUiThread
                    val v = x.version()
                    x.takeTracking(bits)
                    if (x.version() != v) opticalChanged(m, x, name)
                }
            }
        }
        scan(QrCamera.Facing.SELFIE, name, continuous = true) { bytes ->
            runOnUiThread {
                if (m.step() != Meet.Step.OPTICAL || m.exchange !== x) return@runOnUiThread
                // **news is the exchange's version moving, not a verdict
                // and not a count**: a header's `got` is taken before the
                // part is judged a duplicate, and a header can complete
                // the exchange with neither count moving. The exchange
                // says whether anything changed; this side acts on that.
                val v = x.version()
                when (x.take(bytes)) {
                    // a frame that reads but is not well formed is a stray —
                    // a reflection, a half-lit code, somebody else's screen
                    // — and the next frame is a second's wait away; a
                    // meeting that has in fact died ends on staleness
                    OpticalExchange.Took.MALFORMED -> {
                        Diag.warn("optical.stray", "which" to name, "bytes" to bytes.size)
                        return@runOnUiThread
                    }
                    // said rather than silent, so a run shows how often a
                    // phone reads itself off the other's glass
                    OpticalExchange.Took.MIRROR -> {
                        Diag.event("optical.mirror", "which" to name, "index" to (bytes[2].toInt() and 0xff))
                        return@runOnUiThread
                    }
                    else -> {}
                }
                if (x.version() == v) return@runOnUiThread
                opticalChanged(m, x, name)
            }
        }
    }

    /**
     * **The exchange moved**, by a read or by the rows, the one place the
     * screen acts on it: the staleness clock restarts, the move is on the
     * record, and where both sides hold everything the object goes to the
     * kernel — the exchange stays so its last code stays up until the
     * kernel moves on.
     */
    private fun opticalChanged(m: Meet, x: OpticalExchange, name: String) {
        m.opticalMoved()
        // **the shape goes to this side's nominees as it happens** (kind
        // 21) [author, 2026-10-19]: one message per event, never batched,
        // because the arrival pace on their clocks is what they attest
        // from — the counts once both are known, then each inbound part as
        // it lands and each outbound part their display acknowledges.
        // Never the content: a part is half of what keys the session
        Kernel.opticalTrace(x.drainTrace())
        Diag.event("optical.exchange", "which" to name, "showing" to x.showing(), "received" to x.received(), "theirs" to x.theirCount(), "they_hold" to x.theirReceived())
        if (x.done() && m.handOff(x)) {
            camera?.close()
            val why = Kernel.takeOptical(x.theirs() ?: return)
            if (why != null) { m.stop(why); return }
        }
        redraw()
    }

    // ---- D4 capture: the counterparty's face ---------------------------

    /**
     * **The window** (design §7.1 items 4–6), one screen: the capture runs
     * on the selfie camera the phone is already pointing at the
     * counterparty, turned by the kernel's hold from a thread of this
     * shell's, while the verifier selection, the queries and the
     * conversation run beside it. The disclosures were read at D1.5 and the
     * device faces away, so this shows state and takes nothing but a stop.
     * Developer wording until the UX pass [author, 2026-10-24].
     */
    private fun window(m: Meet) {
        para("Capturing ${m.counterpartyName}: a few frames over a few seconds, with spoken or toned prompts for them. Nothing is shown to you and nothing is asked — you read what this holds and who may see it before the phone turned around.")
        if (!held(Manifest.permission.CAMERA)) {
            para("The capture needs the camera, which is not yet allowed.")
            button("Allow the camera") { ask(2, Manifest.permission.CAMERA) }
            return
        }
        once(m, "window") {
            // the capture key crosses and the hold is turned here; the
            // window itself opens from the kernel's drain as the keep lands,
            // and the conversation with it
            Kernel.runWindow(m)
        }
        para("state: capture ${if (m.isCaptureSealed()) "sealed" else "running"}; window ${if (m.windowOpen()) "open" else "not yet open"}")
        verification(m)
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

    private fun verification(m: Meet) {
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
                para("No witness attests this meeting — nobody could be nominated, or none agreed. The record still finalises, and says so permanently; anyone reviewing it later will see an unwitnessed meeting and weigh it as one.")
            p.proposer && m.waitingOn() != null -> para("Not yet proposed: waiting on ${m.waitingOn()}.")
            p.proposer -> para("Proposing the body to every signer.")
            else -> para("Your gathered responses are with ${m.counterpartyName}, whose device proposes the body. It is shown here when it arrives, reviewed and signed by this device as it does.")
        }
        button("Stop") { m.stop("you stopped inside the window") }
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
        // **the verifiers' answers are in the body too** (`wire-format.md`
        // §4.5 field 5), and a person signing it is owed sight of them; the
        // first verified record on the bench was signed from a screen that
        // named none (run 8bc1291-b1-node-1, 2026-10-25)
        val chosen = m.chosen()
        val mine = m.queriesAboutMe()
        if (chosen.isNotEmpty() || mine.isNotEmpty()) {
            heading("Verifiers")
            chosen.forEach { c ->
                val answer = when (c.verdict) {
                    null -> "no answer; absent from the record"
                    Meet.Verdict.MATCH -> "answered: a match"
                    Meet.Verdict.NO_MATCH -> "answered: no match"
                    Meet.Verdict.INCONCLUSIVE -> "answered: inconclusive"
                    Meet.Verdict.UNAVAILABLE -> "unavailable; silence counts for nothing either way"
                }
                para("• ${c.key.take(16)}…  asked about ${m.counterpartyName}, ${basisWords(c.basis)}; $answer")
            }
            mine.forEach { para("• $it…  asked about you by ${m.counterpartyName}; its answer is ${m.counterpartyName}'s to see") }
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
        // PRD-05: two people who each chose to be the patron have not hit
        // a protocol failure, and the screen must not say they have
        if (m.opposedAdoptions() && m.direction() == null) {
            heading("You both offered to be the patron")
            para("Each of you chose a direction before seeing the other's, so this is two intentions rather than a fault — nothing malformed has happened, and the meeting itself stands whichever way this goes.")
            para("Choose a direction between you, or leave the authority question alone.")
            button("I will be the patron") {
                m.chooseDirection(Meet.Adopt.THEM_UNDER_ME)
                Kernel.adoptionBegins()
            }
            button("${m.counterpartyName} will be the patron") {
                m.chooseDirection(Meet.Adopt.ME_UNDER_THEM)
                Kernel.adoptionBegins()
            }
            button("Neither — just the meeting") {
                m.chooseDirection(Meet.Adopt.NONE)
                Kernel.adoptionBegins()
            }
            return
        }
        witnessed(m)
        adoption(m, Kernel.adoption())
        peering(m, Kernel.peering())
        button("Done") { Kernel.stopMeet("done"); finish() }
    }

    /**
     * **Who witnessed it, and whose nominees they were** [author,
     * 2026-10-19]. The survivors of the prune that signed, split by the
     * side that nominated each — a record attested only by the
     * counterparty's survivors is one this side of the story did not
     * witness, which the person is owed sight of (design §6.1.1, §7.1.1).
     *
     * **A witnessless record is said plainly, not apologised for**: there
     * is no floor, the absence is in the signed body for every later
     * reader, and a patron reviewing this archive will read it as such
     * (`wire-format.md` §3.2).
     */
    private fun witnessed(m: Meet) {
        val signed = m.attested()
        if (signed.isEmpty()) {
            para("Nobody witnessed this meeting. The record says so, permanently — the two of you are its only signers.")
            return
        }
        val mine = signed.count { it.mine }
        val theirs = signed.size - mine
        para(
            "Witnessed by ${signed.size}: $mine nominated by you, $theirs by ${m.counterpartyName}." +
                if (mine == 0) " Your own nominees are absent from it." else "",
        )
    }

    /**
     * **The peering, which the meeting can carry alongside anything else**
     * (A3; design §6.3): an edge between two parties where neither is over
     * the other, priced in the same meeting the record above rests on.
     *
     * **Asked for at the invitation, not here** [author, 2026-10-19]: the
     * checkbox on the regular meeting is where the two choose it, as the
     * adoption's direction is chosen there, so this section reports what
     * it came to. A peering has no direction, so there was nothing else
     * for the two to settle. It is **not an alternative to the adoption**
     * — a meeting can carry both — so this sits beside that section
     * rather than replacing it, and a meeting that asked for none shows
     * nothing at all.
     */
    private fun peering(m: Meet, p: Peering?) {
        if (!m.peeringAsked() && p == null) return
        heading("A peering with ${m.counterpartyName}")
        if (p == null) {
            para("The peering is next: this screen will say what it came to.")
            return
        }
        when (p.step()) {
            Peering.Step.HELD -> {
                para("The peering is recorded: ${p.txid()}")
                para("Your device will hand the record to its node when it has one. Until then it is held here, which is where it is evidence.")
            }
            Peering.Step.REFUSED ->
                para("The peering did not happen: ${p.reason()}. The meeting itself is untouched — the record above stands, and either of you can offer again.")
            else -> para("Waiting on ${m.counterpartyName}'s device.")
        }
    }

    /** What the adoption the meeting carried came to, where it carried one. */
    private fun adoption(m: Meet, a: Adoption?) {
        if (m.direction() == Meet.Adopt.NONE && a == null) {
            para("You left the authority question alone. The meeting stands on its own, which costs neither of you anything you had.")
            return
        }
        if (a == null) {
            if (m.adopt != Meet.Adopt.NONE) {
                para("The adoption is next: this screen will say what it came to.")
            }
            return
        }
        val into = (a.into() ?: m.subnetLabel())?.let { " — $it" } ?: ""
        heading(
            when (a.direction()) {
                Meet.Adopt.THEM_UNDER_ME -> "${m.counterpartyName} under you$into"
                Meet.Adopt.ME_UNDER_THEM -> "You under ${m.counterpartyName}$into"
                else -> "The authority question"
            },
        )
        when (a.step()) {
            Adoption.Step.HELD -> {
                val t = a.txid()
                if (t == null) {
                    para("Nothing was proposed, and nothing is owed: the meeting stands on its own.")
                } else {
                    para("The adoption is recorded: $t")
                    para("Where you sit now: ${a.position()}")
                    para("Your device will hand the record to its node when it has one. Until then it is held here, which is where it is evidence.")
                }
            }
            Adoption.Step.REFUSED ->
                para("The adoption did not happen: ${a.reason()}. The meeting itself is untouched — the record above stands, and the two of you can try the adoption again from either side.")
            else -> para("Waiting on ${m.counterpartyName}'s device.")
        }
    }

    private fun stopped(m: Meet) {
        para("Stopped: ${m.stopReason()}")
        button("Back") { Kernel.stopMeet("dismissed"); finish() }
    }

    // ---- view helpers --------------------------------------------------

    private fun title(t: String) {
        if (optical) return
        titleAlways(t)
    }

    private fun titleAlways(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 24f
            typeface = Typeface.DEFAULT_BOLD
            setPadding(0, 0, 0, 24)
        },
    )

    private fun heading(t: String) {
        if (optical) return
        headingAlways(t)
    }

    private fun headingAlways(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 16f
            typeface = Typeface.DEFAULT_BOLD
            setPadding(0, 20, 0, 8)
        },
    )

    /**
     * **Nothing but the code and its rows while the exchange runs**
     * [author, 2026-10-07]: the prose is extraneous and the screen space
     * is not spare. What the words described — the aiming band, the part
     * count, the progress — the band itself and the tracking rows show
     * directly, and every line of it costs module size, which costs
     * range.
     */
    private fun para(t: String) {
        if (optical) return
        paraAlways(t)
    }

    private fun paraAlways(t: String) = body.addView(
        TextView(this).apply {
            text = t
            textSize = 15f
            setPadding(0, 8, 0, 8)
        },
    )

    /** A control is not drawn while the exchange is on screen either:
     *  there is nothing to press until it ends, and the space is the
     *  code's ([para]). The one exception is a refused camera, which is
     *  drawn through [buttonAlways] because without it the screen is a
     *  dead end. */
    private fun button(label: String, onClick: () -> Unit) {
        if (optical) return
        buttonAlways(label, onClick)
    }

    private fun buttonAlways(label: String, onClick: () -> Unit) = body.addView(
        Button(this).apply {
            text = label
            setPadding(0, 16, 0, 16)
            setOnClickListener { onClick() }
        },
    )

    /**
     * **The page stays where the person's content is: the top.**
     *
     * The ceremony's running commentary used to be drawn here, in grey,
     * and the page was scrolled to the bottom to keep its newest line in
     * sight — which put the controls above the fold on every render
     * [author, 2026-10-18]. The commentary was for reading a bench run
     * off a screen, and a bench run is read from the diagnostic stream
     * now (`Meet.note` emits every line to it, as `Adoption` and
     * `Peering` do), so the screen keeps none of it and nothing is lost.
     * What the person needs is the step they are in, what it is waiting
     * on, and the controls — all of which are at the top.
     */
    private fun atTop() = scroll.post { scroll.fullScroll(ScrollView.FOCUS_UP) }
}
