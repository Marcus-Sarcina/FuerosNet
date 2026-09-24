package com.comptus.fueros

import android.app.Activity
import android.graphics.Typeface
import android.os.Bundle
import android.view.Gravity
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import java.io.File
import java.security.SecureRandom
import org.json.JSONObject
import uniffi.rhtn_ffi.Event
import uniffi.rhtn_ffi.Participant
import uniffi.rhtn_ffi.Refused
import uniffi.rhtn_ffi.Status
import uniffi.rhtn_ffi.kindApplication

/**
 * The payload screen: the kernel's status, what arrived, and a box to
 * send from.
 *
 * **This device mints its own identity and keeps it.**  The seeds are made
 * here on first launch and never leave; what travels is the public half,
 * printed for whoever must admit this device.  A provision blob — the
 * `provision` intent extra, or a file of the same name in the kernel's
 * storage — then names the node, where it serves, and the peer to talk to,
 * every field of it public.  Until one arrives the screen starts the
 * kernel, shows what it presents, and can reach no one.
 *
 * All of this is the ceremony's stand-in.  The ceremony is how two devices
 * are actually introduced; this is a hand-carried substitute for it while
 * the payload path is what is being built.
 */
class MainActivity : Activity() {

    private lateinit var status: TextView
    private lateinit var messages: TextView
    private lateinit var scroll: ScrollView
    @Volatile private var participant: Participant? = null
    @Volatile private var peer: ByteArray? = null
    private var peerName: String = "peer"

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        status = TextView(this).apply {
            textSize = 14f
            typeface = Typeface.MONOSPACE
            text = "starting the kernel…"
        }
        messages = TextView(this).apply { textSize = 16f }
        scroll = ScrollView(this).apply {
            addView(messages)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f
            )
        }
        val box = EditText(this).apply {
            hint = "payload"
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
        }
        val send = Button(this).apply { text = "Send" }
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(box)
            addView(send)
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            fitsSystemWindows = true
            setPadding(32, 16, 32, 16)
            addView(status)
            addView(scroll)
            addView(row)
        }
        setContentView(root)

        send.setOnClickListener {
            val text = box.text.toString()
            if (text.isEmpty()) return@setOnClickListener
            val p = participant
            val to = peer
            if (p == null || to == null) {
                say("· unprovisioned: nobody to send to")
                return@setOnClickListener
            }
            box.setText("")
            say("me: $text")
            Thread {
                try {
                    p.send(to, kindApplication(), text.toByteArray())
                } catch (e: Refused.Reason) {
                    runOnUiThread { say("· refused: ${e.reason}") }
                }
            }.start()
        }

        intent.getStringExtra("provision")?.let {
            AndroidShell(this).write("provision", it.toByteArray())
        }
        Thread { bringUp() }.start()
    }

    /** Start the kernel, attach where provisioned, and pump its events. */
    private fun bringUp() {
        val shell = AndroidShell(this)
        val provision = shell.read("provision")?.let {
            try { JSONObject(String(it)) } catch (_: Exception) { null }
        }
        // the identity is this device's own, whether or not anybody has
        // been told about it yet
        val known = provision?.getJSONArray("known")
            ?.let { a -> (0 until a.length()).map { unhex(a.getString(it)) } }
            ?: listOf()
        val p: Participant
        try {
            p = Participant.start(mintedSeeds(), known, platformOf(this))
            participant = p
            show(p, Status.Detached)
        } catch (e: Refused.Reason) {
            runOnUiThread { status.text = "kernel refused: ${e.reason}" }
            return
        }

        // the public half, for whoever must admit this device
        val material = p.material().joinToString("") { "%02x".format(it) }
        android.util.Log.i("fueros", "material $material")

        if (provision == null) {
            runOnUiThread {
                say("· unprovisioned. On the workstation:")
                say("    adb logcat -d -s fueros | grep material")
                say("    cargo run -p rhtn-ffi --features harness \\")
                say("      --bin payload-peer -- <that material>")
                say("· then hand its PROVISION line back as the `provision` extra.")
            }
            return
        }

        try {
            peer = unhex(provision.getString("peer"))
            peerName = provision.optString("peer_name", "peer")
            val node = unhex(provision.getString("node"))
            val a = p.attach(node, listOf(provision.getString("addr")), listOf(peer!!))
            show(p, Status.Attached(node, a.primary))
        } catch (e: Refused.Reason) {
            runOnUiThread { status.text = "attach refused: ${e.reason}" }
            return
        }
        while (true) {
            when (val e = p.nextEvent(2000UL)) {
                is Event.Payload -> runOnUiThread { say("$peerName: ${String(e.bytes)}") }
                is Event.Connection -> show(p, e.v1)
                null -> {}
                else -> runOnUiThread { say("· $e") }
            }
        }
    }

    private fun show(p: Participant, s: Status) {
        val key = p.presentedKey().joinToString("") { "%02x".format(it) }.take(16)
        val line = when (s) {
            is Status.Detached -> "detached"
            is Status.Attached -> if (s.primary) "attached, primary" else "attached"
            is Status.Reconnecting -> "reconnecting…"
            is Status.Lost -> "lost: no serving node reachable"
        }
        runOnUiThread { status.text = "$line · presents $key…" }
    }

    private fun say(line: String) {
        messages.append(line + "\n")
        scroll.post { scroll.fullScroll(ScrollView.FOCUS_DOWN) }
    }

    private fun unhex(s: String): ByteArray =
        ByteArray(s.length / 2) { ((s[2 * it].digitToInt(16) shl 4) + s[2 * it + 1].digitToInt(16)).toByte() }

    private fun mintedSeeds(): ByteArray {
        val f = File(File(filesDir, "kernel"), "seeds")
        if (f.isFile) return f.readBytes()
        val s = ByteArray(64).also { SecureRandom().nextBytes(it) }
        f.parentFile?.mkdirs()
        f.writeBytes(s)
        return s
    }
}
