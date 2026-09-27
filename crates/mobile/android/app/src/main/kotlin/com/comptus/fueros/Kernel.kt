package com.comptus.fueros

import android.content.Context
import java.security.SecureRandom
import org.json.JSONObject
import uniffi.rhtn_ffi.Event
import uniffi.rhtn_ffi.Participant
import uniffi.rhtn_ffi.Refused
import uniffi.rhtn_ffi.Status
import uniffi.rhtn_ffi.kindApplication

/**
 * The kernel, owned by the process and not by any screen.
 *
 * An Activity is recreated on a font change; a Participant holds a
 * connection, an event loop and a maintenance clock, and must not be. One
 * kernel is started here once, screens bind to it and unbind from it, and
 * a screen that has gone is a null sink rather than a leaked loop. Every
 * context used is the application's, so nothing here retains an Activity.
 */
object Kernel {

    /** What a bound screen renders. Calls arrive on the kernel's thread. */
    interface Ui {
        fun status(line: String)

        fun say(line: String)
    }

    private val lock = Any()
    @Volatile private var ui: Ui? = null
    @Volatile private var participant: Participant? = null
    @Volatile private var peer: ByteArray? = null
    private var peerName: String = "peer"
    private var started = false
    private var provisioned = false
    private var statusLine = "starting the kernel…"
    private val transcript = mutableListOf<String>()

    /**
     * Bind a screen: it receives the state so far, then everything new.
     *
     * Under the lock, so that a line the kernel is saying at this moment
     * arrives either in the replay or after it and never in both places or
     * out of order.
     */
    fun bind(u: Ui) {
        synchronized(lock) {
            ui = u
            u.status(statusLine)
            transcript.forEach(u::say)
        }
    }

    fun unbind(u: Ui) {
        synchronized(lock) {
            if (ui === u) {
                ui = null
            }
        }
    }

    /**
     * Start the kernel, once for the life of the process.
     *
     * **A second call does nothing**, which is the whole point: a recreated
     * screen finds the kernel it left rather than starting another against
     * the same state files.
     */
    fun start(context: Context) {
        val app = context.applicationContext
        synchronized(lock) {
            if (started) return
            started = true
        }
        Thread({ bringUp(AndroidShell(app)) }, "fueros-kernel").start()
    }

    /**
     * Keep a provision blob for the kernel to read when it comes up.
     *
     * Written before [start] on a cold launch, which is where a provision
     * arrives. **A kernel already up does not re-read it**: it is attached
     * where it was told to attach, and the screen says so rather than
     * appearing to have taken an instruction it did not.
     */
    fun provision(context: Context, blob: String) {
        AndroidShell(context.applicationContext).write("provision", blob.toByteArray())
        synchronized(lock) {
            if (!started || provisioned) return
        }
        say("· provision stored. Restart the app for it to take effect.")
    }

    /**
     * Send application bytes to the provisioned peer.
     *
     * Off the caller's thread: the kernel's own send blocks on the network.
     */
    fun send(text: String) {
        val p = participant
        val to = peer
        if (p == null || to == null) {
            say("· unprovisioned: nobody to send to")
            return
        }
        say("me: $text")
        Thread {
            try {
                p.send(to, kindApplication(), text.toByteArray())
            } catch (e: Refused.Reason) {
                say("· refused: ${e.reason}")
            }
        }.start()
    }

    /** Start the participant, attach where provisioned, and pump its events. */
    private fun bringUp(shell: AndroidShell) {
        val provision = shell.read("provision")?.let {
            try { JSONObject(String(it)) } catch (_: Exception) { null }
        }
        synchronized(lock) { provisioned = provision != null }
        // the identity is this device's own, whether or not anybody has
        // been told about it yet
        val known = provision?.getJSONArray("known")
            ?.let { a -> (0 until a.length()).map { unhex(a.getString(it)) } }
            ?: listOf()
        val p: Participant
        try {
            p = Participant.start(mintedSeeds(shell), known, platformOf(shell))
            participant = p
            show(p, Status.Detached)
        } catch (e: Refused.Reason) {
            status("kernel refused: ${e.reason}")
            return
        }

        // the public half, for whoever must admit this device
        val material = p.material().joinToString("") { "%02x".format(it) }
        android.util.Log.i("fueros", "material $material")

        if (provision == null) {
            say("· unprovisioned. On the workstation:")
            say("    adb logcat -d -s fueros | grep material")
            say("    cargo run -p rhtn-ffi --features harness \\")
            say("      --bin payload-peer -- <that material>")
            say("· then hand its PROVISION line back as the `provision` extra.")
            return
        }

        try {
            peer = unhex(provision.getString("peer"))
            peerName = provision.optString("peer_name", "peer")
            val node = unhex(provision.getString("node"))
            val a = p.attach(node, listOf(provision.getString("addr")), listOf(peer!!))
            show(p, Status.Attached(node, a.primary))
        } catch (e: Refused.Reason) {
            status("attach refused: ${e.reason}")
            return
        }
        while (true) {
            when (val e = p.nextEvent(2000UL)) {
                is Event.Payload -> say("$peerName: ${String(e.bytes)}")
                is Event.Connection -> show(p, e.v1)
                null -> {}
                else -> say("· $e")
            }
        }
    }

    /**
     * The connection, what this device presents, and which construction the
     * payload session runs.
     *
     * **The construction is the client's own statement** and nothing a peer
     * can check (`light-client-requirements.md` §3): the session derives the
     * same keys or it does not, so the only party who can say which half is
     * running is this one, and it says it where its user can read it.
     */
    private fun show(p: Participant, s: Status) {
        val key = p.presentedKey().joinToString("") { "%02x".format(it) }.take(16)
        val line = when (s) {
            is Status.Detached -> "detached"
            is Status.Attached -> if (s.primary) "attached, primary" else "attached"
            is Status.Reconnecting -> "reconnecting…"
            is Status.Lost -> "lost: no serving node reachable"
        }
        status("$line · presents $key…\npayload: ${p.payloadConstruction()}")
    }

    // **Under the lock, and the bound screen told inside it.**  What a
    // screen has been told and what the transcript holds then cannot part
    // company across a bind, which is the only thing that makes the replay
    // exact.  A sink posts to its own thread and never calls back in here,
    // so nothing waits on anything.
    private fun status(line: String) {
        synchronized(lock) {
            statusLine = line
            ui?.status(line)
        }
    }

    private fun say(line: String) {
        synchronized(lock) {
            transcript.add(line)
            // the transcript outlives every screen now, so it is bounded:
            // an unbounded one is a leak the process never recovers from
            while (transcript.size > TRANSCRIPT_LINES) {
                transcript.removeAt(0)
            }
            ui?.say(line)
        }
    }

    private fun unhex(s: String): ByteArray =
        ByteArray(s.length / 2) { ((s[2 * it].digitToInt(16) shl 4) + s[2 * it + 1].digitToInt(16)).toByte() }

    /**
     * The device's own seeds, minted once.
     *
     * **Through the shell's storage and never around it**: these are the
     * whole of the identity, so writing them with plain file I/O would put
     * the most sensitive thing on the device in the one place the at-rest
     * encryption does not reach (`light-client-requirements.md` §9).
     */
    private fun mintedSeeds(shell: AndroidShell): ByteArray {
        shell.read("seeds")?.let { if (it.size == 64) return it }
        val s = ByteArray(64).also { SecureRandom().nextBytes(it) }
        check(shell.write("seeds", s)) { "the device's own storage refused the seeds" }
        return s
    }

    private const val TRANSCRIPT_LINES = 500
}
