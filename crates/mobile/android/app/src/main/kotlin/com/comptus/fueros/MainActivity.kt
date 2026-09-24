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
 * send from.  Identity and addresses come from a provision blob — the
 * `provision` intent extra, kept once seen — which is the ceremony's
 * stand-in and nothing more: seeds minted here belong to a participant
 * nobody knows, so without a provision the screen starts the kernel,
 * shows what it presents, and can reach no one.
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
        val p: Participant
        try {
            if (provision != null) {
                val known = provision.getJSONArray("known")
                    .let { a -> (0 until a.length()).map { unhex(a.getString(it)) } }
                p = Participant.start(unhex(provision.getString("seeds")), known, platformOf(this))
                participant = p
                peer = unhex(provision.getString("peer"))
                peerName = provision.optString("peer_name", "peer")
                show(p, Status.Detached)
                val a = p.attach(
                    unhex(provision.getString("node")),
                    listOf(provision.getString("addr")),
                    listOf(peer!!),
                )
                show(p, Status.Attached(unhex(provision.getString("node")), a.primary))
            } else {
                p = Participant.start(mintedSeeds(), listOf(), platformOf(this))
                participant = p
                show(p, Status.Detached)
                runOnUiThread { say("· unprovisioned: run payload-peer on the workstation and relaunch with its PROVISION line as the `provision` extra") }
                return
            }
        } catch (e: Refused.Reason) {
            runOnUiThread { status.text = "kernel refused: ${e.reason}" }
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
