package com.comptus.fueros

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import android.graphics.SurfaceTexture
import android.os.Bundle
import android.view.Surface
import android.view.TextureView
import android.view.View
import android.widget.Button
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import java.io.DataInputStream
import java.io.DataOutputStream
import java.net.InetSocketAddress
import java.net.Socket
import uniffi.rhtn_ffi.DeviceLink
import uniffi.rhtn_ffi.Participant

/**
 * **Pair a desktop of this identity** ([Desktop]; `wire-format.md`
 * §14.3.3): read the code the desktop shows, then on one connection to
 * where it says, sign its run and its bundle and bring it to the dataset.
 * Every byte that crosses is the kernel's, sealed by the [DeviceLink] the
 * code made; this screen moves them and reads none.
 */
class DesktopActivity : Activity() {
    /** The identity this screen acts for, named by whoever opened it. */
    private lateinit var by: String
    private val lock = Any()
    @Volatile private var step = Desktop.Step.CODE
    @Volatile private var reason: String? = null
    @Volatile private var handed: Boolean? = null
    @Volatile private var socket: Socket? = null
    private var camera: QrCamera? = null
    private var previewSurface: Surface? = null
    private val work = java.util.concurrent.Executors.newSingleThreadExecutor { r ->
        Thread(r, "desktop").apply { isDaemon = true }
    }

    private lateinit var narration: TextView
    private lateinit var status: TextView
    private lateinit var previewHost: FrameLayout
    private lateinit var buttons: LinearLayout

    private companion object {
        const val CAMERA_ASK = 1
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        by = intent.getStringExtra("by") ?: run {
            Diag.warn("desktop.opened", "why" to "no identity named")
            finish()
            return
        }
        val g = Screen.gutter(this)
        narration = TextView(this).apply { textSize = 15f; setTextColor(Palette.now.ink) }
        status = TextView(this).apply { textSize = 13f; setTextColor(Palette.now.sub) }
        buttons = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        val tv = TextureView(this)
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
            addView(tv, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, full * 3 / 4))
        }
        val body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(g, g, g, g)
            addView(narration)
            addView(status)
            addView(previewHost)
            addView(buttons)
        }
        val root = ScrollView(this).apply { addView(body) }
        Screen.insetting(root)
        setContentView(root)
        Diag.event("desktop.opened")
        draw()
    }

    override fun onResume() {
        super.onResume()
        Consent.host(this)
    }

    override fun onDestroy() {
        super.onDestroy()
        camera?.close()
        runCatching { socket?.close() }
        work.shutdownNow()
    }

    private fun participant(): Participant? = Gateway.loaded[by] ?: run {
        stop("this identity is not running")
    }

    private fun draw() {
        runOnUiThread {
            narration.text = Desktop.narration(step)
            status.text = when (step) {
                Desktop.Step.STOPPED -> reason ?: ""
                Desktop.Step.DONE -> when (handed) {
                    true -> "It holds the run and everything this phone holds."
                    else -> "It holds the run. Nothing else was handed over."
                }
                else -> ""
            }
            previewHost.visibility = if (step == Desktop.Step.CODE) View.VISIBLE else View.GONE
            buttons.removeAllViews()
            when (step) {
                Desktop.Step.CODE -> {
                    if (!held(Manifest.permission.CAMERA)) {
                        buttons.addView(Button(this).apply {
                            text = "Allow the camera"
                            setOnClickListener { requestPermissions(arrayOf(Manifest.permission.CAMERA), CAMERA_ASK) }
                        })
                    } else {
                        scan()
                    }
                }
                Desktop.Step.DONE, Desktop.Step.STOPPED -> {
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
        draw()
    }

    private fun held(permission: String) = checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED

    private fun scan() {
        val surface = previewSurface ?: return
        val cam = camera ?: QrCamera(this).also { camera = it }
        val onFailed: (String) -> Unit = { why -> stop("the camera: $why") }
        cam.readOne(QrCamera.Facing.REAR, surface, "desktop", onFailed) { bytes ->
            work.execute { codeRead(bytes) }
        }?.let { why -> status.post { status.text = why } }
    }

    // ---- the steps -------------------------------------------------------

    private fun codeRead(code: ByteArray) {
        if (step != Desktop.Step.CODE) return
        val p = participant() ?: return
        val link = Gateway.call("DeviceLink.read", { e -> stop("not a desktop's code: ${e.reason}"); null }) {
            DeviceLink.read(code)
        } ?: return
        val (host, port) = Desktop.hostPort(link.reach()) ?: run {
            stop("the code names no address: ${link.reach()}")
            return
        }
        runOnUiThread { camera?.close(); camera = null }
        advance(Desktop.Step.CREDENTIAL)
        val s = Socket()
        socket = s
        try {
            s.connect(InetSocketAddress(host, port), Desktop.STEP_MS)
            s.soTimeout = Desktop.STEP_MS
            val input = DataInputStream(s.getInputStream().buffered())
            val out = DataOutputStream(s.getOutputStream().buffered())
            run(p, link, input, out)
        } catch (e: java.io.IOException) {
            stop("the connection to the desktop: ${e.message}")
        } finally {
            runCatching { s.close() }
        }
    }

    private fun run(p: Participant, link: DeviceLink, input: DataInputStream, out: DataOutputStream) {
        val notBefore = System.currentTimeMillis() / 1000 - Desktop.LEAD_S
        val start = Gateway.call("deviceStart", { e -> stop("no run was signed: ${e.reason}"); null }) {
            p.deviceStart(link, notBefore.toULong(), Desktop.RUN.toUInt())
        } ?: return
        Desktop.write(out, start)
        val intro = Desktop.read(input)
        val credential = Gateway.call("deviceCredential", { e -> stop("the desktop's introduction was refused: ${e.reason}"); null }) {
            p.deviceCredential(link, intro)
        } ?: return
        Desktop.write(out, credential)
        Diag.event("desktop.credential", "windows" to Desktop.RUN)

        advance(Desktop.Step.DATASET)
        // shown before the other's is taken: showing ends the pairing before
        val mine = Gateway.call("datasetContribution", { e -> stop("no contribution: ${e.reason}"); null }) {
            p.datasetContribution()
        } ?: return
        val theirs = Gateway.call("DeviceLink.open", { e -> stop("the desktop's contribution: ${e.reason}"); null }) {
            link.open(Desktop.read(input))
        } ?: return
        Gateway.call("takeDatasetContribution", { e -> stop("the desktop's contribution was refused: ${e.reason}"); null }) {
            p.takeDatasetContribution(theirs)
        } ?: return
        Desktop.write(out, link.seal(mine))
        // the kernel asks this phone's holder before composing anything; a
        // no closes the connection, which the desktop reads as no dataset
        val sealed = Gateway.call("datasetCarriage", { _ -> null }) { p.datasetCarriage() }
        if (sealed != null) {
            Desktop.write(out, link.seal(sealed))
            Diag.event("desktop.handed", "bytes" to sealed.size)
        }
        handed = sealed != null
        advance(Desktop.Step.DONE)
    }

    private fun advance(to: Desktop.Step) {
        synchronized(lock) {
            if (step == Desktop.Step.STOPPED) return
            step = to
        }
        Diag.event("desktop.step", "step" to to.name)
        draw()
    }

    private fun stop(why: String): Nothing? {
        synchronized(lock) {
            if (step == Desktop.Step.STOPPED) return null
            step = Desktop.Step.STOPPED
            reason = why
        }
        Diag.warn("desktop.stopped", "why" to why)
        runOnUiThread {
            camera?.close()
            camera = null
        }
        runCatching { socket?.close() }
        draw()
        return null
    }
}
