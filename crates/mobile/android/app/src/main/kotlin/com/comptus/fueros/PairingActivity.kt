package com.comptus.fueros

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.SurfaceTexture
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.Surface
import android.view.TextureView
import android.view.View
import android.widget.Button
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import uniffi.rhtn_ffi.DatasetTaken
import uniffi.rhtn_ffi.Participant

/**
 * **Another device of this identity, brought to the dataset** (design
 * §23.3; `light-client-requirements.md` §4.2; `wire-format.md` §14.3.3;
 * APP-014, APP-017).
 *
 * The kernel has had the hand-over since the ruling of 2026-11-15 — the
 * pairing by contribution, the question to each holder, the dataset
 * sealed under the pairing — and the shell had no way in. This is the
 * way in, shaped on the meeting: each device shows a code and reads the
 * other's, the radio link forms under the pairing's id, and the sealed
 * dataset crosses it once each holder has said yes on their own device.
 *
 * **What this screen never does.** It parses none of the bytes it moves
 * (NET-019), it offers the dataset to no network path, and it answers
 * the kernel's question for nobody: the question reaches the person as
 * a dialog on the device it concerns, through [Consent].
 */
class PairingActivity : Activity() {
    /** The identity this screen acts for, named by whoever opened it. */
    private lateinit var by: String
    private val lock = Any()
    @Volatile private var step = Pairing.Step.ROLE
    @Volatile private var role: Pairing.Role? = null
    @Volatile private var reason: String? = null
    @Volatile private var outcome: DatasetTaken? = null
    @Volatile private var exchange: OpticalExchange? = null
    @Volatile private var pairingId: ByteArray? = null
    @Volatile private var radio: BleBearer? = null
    @Volatile private var carriage: Carriage? = null
    private var camera: QrCamera? = null
    private var preview: TextureView? = null
    private var previewSurface: Surface? = null
    private val work = java.util.concurrent.Executors.newSingleThreadExecutor { r ->
        Thread(r, "pairing").apply { isDaemon = true }
    }
    private val turning = Handler(Looper.getMainLooper())
    private var turner: Runnable? = null

    private lateinit var narration: TextView
    private lateinit var status: TextView
    private lateinit var code: ImageView
    private lateinit var previewHost: FrameLayout
    private lateinit var buttons: LinearLayout

    private companion object {
        const val CAMERA_ASK = 1
        const val RADIO_ASK = 2
        val RADIO = arrayOf(
            Manifest.permission.BLUETOOTH_SCAN,
            Manifest.permission.BLUETOOTH_ADVERTISE,
            Manifest.permission.BLUETOOTH_CONNECT,
        )
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        by = intent.getStringExtra("by") ?: run {
            Diag.warn("pairing.opened", "why" to "no identity named")
            finish()
            return
        }
        val g = Screen.gutter(this)
        narration = TextView(this).apply { textSize = 15f; setTextColor(Palette.now.ink) }
        status = TextView(this).apply { textSize = 13f; setTextColor(Palette.now.sub) }
        code = ImageView(this).apply { visibility = View.GONE; adjustViewBounds = true }
        buttons = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        val tv = TextureView(this).also { preview = it }
        tv.surfaceTextureListener = object : TextureView.SurfaceTextureListener {
            override fun onSurfaceTextureAvailable(st: SurfaceTexture, width: Int, height: Int) {
                st.setDefaultBufferSize(1280, 960)
                previewSurface = Surface(st)
                draw()
            }
            override fun onSurfaceTextureSizeChanged(st: SurfaceTexture, width: Int, height: Int) {}
            override fun onSurfaceTextureUpdated(st: SurfaceTexture) {}
            override fun onSurfaceTextureDestroyed(st: SurfaceTexture): Boolean {
                previewSurface?.release()
                previewSurface = null
                return true
            }
        }
        val full = resources.displayMetrics.widthPixels - 2 * g
        previewHost = FrameLayout(this).apply {
            setBackgroundColor(Palette.now.sunken)
            visibility = View.GONE
            addView(tv, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, full / 3))
        }
        val body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(g, g, g, g)
            addView(narration)
            addView(status)
            addView(previewHost)
            addView(code)
            addView(buttons)
        }
        val root = ScrollView(this).apply { addView(body) }
        Screen.insetting(root)
        setContentView(root)
        Diag.event("pairing.opened")
        draw()
    }

    override fun onResume() {
        super.onResume()
        Consent.host(this)
    }

    override fun onDestroy() {
        super.onDestroy()
        stopTurning()
        camera?.close()
        radio?.close()
        work.shutdownNow()
    }

    private fun participant(): Participant? = Gateway.loaded[by] ?: run {
        stop("this identity is not running")
        null
    }

    // ---- the screen ----------------------------------------------------

    private fun draw() {
        runOnUiThread {
            narration.text = Pairing.narration(step, role)
            status.text = when (step) {
                Pairing.Step.STOPPED -> reason ?: ""
                Pairing.Step.DONE -> outcome?.let { Pairing.taken(it.records, it.contacts, it.other, it.refused) } ?: ""
                Pairing.Step.CODES, Pairing.Step.LINK -> exchange?.let {
                    "showing ${it.showing() + 1}/${it.count}; read ${it.received()} of the other device's ${it.theirCount()}"
                } ?: ""
                else -> ""
            }
            buttons.removeAllViews()
            val showing = step == Pairing.Step.CODES || step == Pairing.Step.LINK
            previewHost.visibility = if (step == Pairing.Step.CODES) View.VISIBLE else View.GONE
            code.visibility = if (showing && exchange != null) View.VISIBLE else View.GONE
            when (step) {
                Pairing.Step.ROLE -> {
                    buttons.addView(Button(this).apply {
                        text = "This device hands over"
                        setOnClickListener { choose(Pairing.Role.HAND) }
                    })
                    buttons.addView(Button(this).apply {
                        text = "This device takes"
                        setOnClickListener { choose(Pairing.Role.TAKE) }
                    })
                }
                Pairing.Step.CODES -> {
                    if (!held(Manifest.permission.CAMERA)) {
                        buttons.addView(Button(this).apply {
                            text = "Allow the camera"
                            setOnClickListener { requestPermissions(arrayOf(Manifest.permission.CAMERA), CAMERA_ASK) }
                        })
                    } else {
                        scan()
                    }
                    exchange?.let { showCode(it) }
                }
                Pairing.Step.LINK -> {
                    if (RADIO.any { !held(it) }) {
                        buttons.addView(Button(this).apply {
                            text = "Allow the radio"
                            setOnClickListener { requestPermissions(RADIO, RADIO_ASK) }
                        })
                    }
                    exchange?.let { showCode(it) }
                }
                Pairing.Step.DONE, Pairing.Step.STOPPED -> {
                    buttons.addView(Button(this).apply {
                        text = "Close"
                        setOnClickListener { finish() }
                    })
                }
                else -> {}
            }
        }
    }

    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == RADIO_ASK && step == Pairing.Step.LINK && radio == null) startLink()
        draw()
    }

    private fun held(permission: String) = checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED

    private fun showCode(x: OpticalExchange) {
        val m = Optical.matrix(x.frame())
        val avail = resources.displayMetrics.widthPixels - 2 * Screen.gutter(this)
        val scale = maxOf(1, (avail - 8) / m.width)
        val w = m.width * scale
        code.setImageBitmap(Bitmap.createBitmap(Optical.pixels(m, scale), w, w, Bitmap.Config.ARGB_8888))
        if (turner == null) turnWhileShowing(x)
    }

    private fun turnWhileShowing(x: OpticalExchange) {
        val r = object : Runnable {
            override fun run() {
                val live = exchange === x && (step == Pairing.Step.CODES || step == Pairing.Step.LINK)
                if (!live) {
                    turner = null
                    return
                }
                x.turn()
                turning.postDelayed(this, Meet.TURN_MS)
                showCode(x)
            }
        }
        turner = r
        turning.postDelayed(r, Meet.TURN_MS)
    }

    private fun stopTurning() {
        turner?.let { turning.removeCallbacks(it) }
        turner = null
    }

    // ---- the steps -------------------------------------------------------

    private fun choose(r: Pairing.Role) {
        role = r
        advance(Pairing.Step.CODES)
        Diag.event("pairing.role", "role" to r.name)
        work.execute {
            val p = participant() ?: return@execute
            val mine = Gateway.call("datasetContribution", { e -> stop("no contribution: ${e.reason}"); null }) {
                p.datasetContribution()
            } ?: return@execute
            exchange = OpticalExchange(OpticalExchange.DATASET, mine)
            draw()
        }
    }

    private fun scan() {
        val x = exchange ?: return
        val surface = previewSurface ?: return
        val cam = camera ?: QrCamera(this).also { camera = it }
        val onFailed: (String) -> Unit = { why -> stop("the camera: $why") }
        cam.readOne(QrCamera.Facing.SELFIE, surface, "pairing", onFailed, continuous = true) { bytes ->
            if (x.take(bytes) == OpticalExchange.Took.COMPLETE) work.execute { theirsRead(x) }
            draw()
        }?.let { why -> status.post { status.text = why } }
    }

    private fun theirsRead(x: OpticalExchange) {
        if (step != Pairing.Step.CODES) return
        val p = participant() ?: return
        val theirs = x.theirs() ?: return
        val heads = Gateway.call("takeDatasetContribution", { e -> stop("their contribution was refused: ${e.reason}"); null }) {
            p.takeDatasetContribution(theirs)
        } ?: return
        val id = Gateway.call("datasetPairingId", { e -> stop("no pairing: ${e.reason}"); null }) {
            p.datasetPairingId()
        } ?: return
        pairingId = id
        Diag.event("pairing.paired", "their_heads" to heads.size)
        advance(Pairing.Step.LINK)
        startLink()
    }

    private fun startLink() {
        val id = pairingId ?: return
        val r = role ?: return
        if (RADIO.any { !held(it) }) {
            draw()
            return
        }
        synchronized(lock) { if (radio != null) return }
        val bearer = BleBearer(this)
        val inbound = BleBearer.Inbound { packet ->
            val live = carriage ?: return@Inbound
            if (work.isShutdown) return@Inbound
            try {
                work.execute {
                    if (carriage !== live) return@execute
                    live.packet(packet)?.let { why -> Diag.warn("pairing.packet", "why" to why) }
                    drain()
                }
            } catch (e: java.util.concurrent.RejectedExecutionException) {
            }
        }
        val why = if (Pairing.offers(r)) bearer.offer(inbound, id) else bearer.seek(inbound, id)
        if (why != null) {
            stop("no radio link: $why")
            return
        }
        synchronized(lock) {
            radio = bearer
            carriage = Carriage(bearer.link())
        }
        Thread({
            val ready = Bearer.awaitReady(Pairing.LINK_READY_MS) { bearer.ready() || step == Pairing.Step.STOPPED }
            if (step == Pairing.Step.STOPPED) return@Thread
            if (!ready) {
                stop("the other device's radio was not found within ${Pairing.LINK_READY_MS / 1000} s")
                return@Thread
            }
            runOnUiThread {
                stopTurning()
                camera?.close()
                camera = null
            }
            advance(Pairing.Step.CARRIAGE)
            Diag.event("pairing.linked", "role" to r.name)
            if (Pairing.offers(r)) work.execute { hand() }
        }, "pairing-link").apply { isDaemon = true }.start()
    }

    private fun hand() {
        val p = participant() ?: return
        val live = carriage ?: return
        // the kernel asks this device's holder before composing anything
        val sealed = Gateway.call("datasetCarriage", { e -> stop("nothing left this device: ${e.reason}"); null }) {
            p.datasetCarriage()
        } ?: return
        if (live.send(listOf(sealed), Pairing.PHASE, wipe = true)) {
            Diag.event("pairing.handed", "bytes" to sealed.size)
            outcome = null
            advance(Pairing.Step.DONE)
            radio?.quiet()
        } else {
            stop("the dataset did not cross the radio")
        }
    }

    private fun drain() {
        if (role != Pairing.Role.TAKE || step != Pairing.Step.CARRIAGE) return
        val live = carriage ?: return
        val sealed = live.received(Pairing.PHASE)?.firstOrNull() ?: return
        live.discard(Pairing.PHASE)
        val p = participant() ?: return
        // the kernel asks this device's holder before opening anything
        val took = Gateway.call("takeDatasetCarriage", { e -> stop("nothing was taken: ${e.reason}"); null }) {
            p.takeDatasetCarriage(sealed)
        } ?: return
        outcome = took
        Diag.event("pairing.taken", "records" to took.records, "contacts" to took.contacts, "other" to took.other, "refused" to took.refused)
        advance(Pairing.Step.DONE)
        radio?.quiet()
    }

    private fun advance(to: Pairing.Step) {
        synchronized(lock) { step = to }
        Diag.event("pairing.step", "step" to to.name)
        draw()
    }

    private fun stop(why: String) {
        synchronized(lock) {
            if (step == Pairing.Step.STOPPED) return
            step = Pairing.Step.STOPPED
            reason = why
        }
        Diag.warn("pairing.stopped", "why" to why)
        runOnUiThread {
            stopTurning()
            camera?.close()
            camera = null
        }
        draw()
    }
}
