package com.comptus.fueros

import android.content.Context
import java.security.SecureRandom
import org.json.JSONObject
import uniffi.rhtn_ffi.Construction
import uniffi.rhtn_ffi.Event
import uniffi.rhtn_ffi.Participant
import uniffi.rhtn_ffi.Refused
import uniffi.rhtn_ffi.Status
import uniffi.rhtn_ffi.Told
import uniffi.rhtn_ffi.kindApplication

/**
 * The kernel, owned by the process and not by any screen.
 *
 * An Activity is recreated on a font change; a Participant holds a
 * connection, an event loop and a maintenance clock, and must not be. One
 * kernel is started here once, screens bind to its [front] and unbind from
 * it, and a screen that has gone is a null sink rather than a leaked loop.
 * Every context used is the application's, so nothing here retains an
 * Activity.
 */
object Kernel {

    /** The state a screen renders. Screens read it and bind to it; only the
     *  kernel mutates it. It lives in [Front], which a JVM test can hold. */
    val front = Front()

    private val lock = Any()
    @Volatile private var participant: Participant? = null
    @Volatile private var peer: ByteArray? = null
    @Volatile private var peerKey: String? = null
    private var peerName: String = "peer"
    private var started = false
    private var provisioned = false

    /** The ceremony in progress, or null. Process-scoped like the
     *  connection: a font change mid-ceremony must not lose it. */
    @Volatile private var meet: Meet? = null

    /** Bind a screen: it is rendered against the state so far, then on
     *  every change. */
    fun bind(u: Front.Ui) = front.bind(u)

    fun unbind(u: Front.Ui) = front.unbind(u)

    fun meet(): Meet? = meet

    /**
     * Begin a ceremony with the one provisioned peer, meeting or adopting
     * as chosen. The kernel prepares the local half; the optical handshake
     * and the bearer that carries the intent are specified now
     * (`wire-format.md` §14.3) but not yet wired in this shell, so this
     * stands the flow up rather than completing it. A ceremony already live
     * is returned as-is.
     */
    fun startMeet(adopt: Meet.Adopt): Meet? {
        val p = participant ?: return null
        val to = peer ?: return null
        val key = peerKey ?: return null
        synchronized(lock) {
            meet?.let { return it }
            meet = Meet(key, peerName, adopt)
        }
        val m = meet!!
        Thread {
            try {
                // witnesses are nominated from the counterparty's
                // neighbourhood (design §7.1); with no horizon yet the
                // nomination is empty and the ceremony is that much weaker,
                // which the record carries honestly rather than hiding
                p.begin(to, listOf(), true)
                m.note("intent prepared. A handshake goes screen-to-screen and")
                m.note("the intent rides a bearer the shell picks (wire-format")
                m.note("§14.3); the carriage is not wired in this build.")
            } catch (e: Refused.Reason) {
                m.stop("begin refused: ${e.reason}")
            }
        }.start()
        return m
    }

    /**
     * Select the counterparty's verifiers and put this device's queries to
     * them (design §8.1.2; `wire-format.md` §5.1–5.5).
     *
     * **The selection is the kernel's and not a choice on a screen**: the
     * tiers are computed from what this device knows and the pool from the
     * records the counterparty handed over, so the screen shows who was
     * picked and why, never a list to choose from.
     *
     * What this build cannot do is **carry a query to its verifier**. A
     * verifier is a third party reached through the network, and the
     * documents stop at its serving node (`wire-format.md` §7.7.2): how
     * that node hands a query to a client attached over the wire is
     * unwritten, and so is how the capture-key grant travels. So the
     * queries are prepared and the selection is real; nothing is sent.
     */
    fun selectVerifiers() {
        val p = participant ?: return
        val m = meet ?: return
        // once per ceremony: a query issued twice is two queries, and the
        // selection is not a thing to re-run on a recreated screen
        if (m.selectionRun()) return
        Thread {
            try {
                val picked = p.selectVerifiers()
                m.selected(
                    picked.map {
                        Meet.Chosen(hex(it.verifier), basisOf(it.basis))
                    },
                )
                if (picked.isEmpty()) {
                    m.note("no verifier is required: the counterparty handed")
                    m.note("over no records, so its pool is empty and")
                    m.note("wire-format §5.2 obliges none.")
                } else {
                    // prepared, to show the call sequence is whole even
                    // where the carriage is not
                    for (s in picked) {
                        p.queryFor(s.verifier)
                    }
                    m.note("${picked.size} query/queries prepared. Carrying one to")
                    m.note("its verifier needs the leg wire-format §7.7.2")
                    m.note("leaves unwritten; nothing was sent.")
                }
            } catch (e: Refused.Reason) {
                m.note("selection refused: ${e.reason}")
            }
        }.start()
    }

    /** `wire-format.md` §5.5's `selection_basis`, as the shell's own words
     *  render it; an unknown value is the discretionary tier, which claims
     *  the least. */
    private fun basisOf(basis: UInt): Meet.Basis = when (basis.toInt()) {
        0 -> Meet.Basis.MET
        1 -> Meet.Basis.IN_HORIZON
        2 -> Meet.Basis.REACHABLE
        else -> Meet.Basis.DISCRETIONARY
    }

    /**
     * A notice the kernel raised, routed where its audience is.
     *
     * **A query about this person is theirs to be told about** (design
     * §7.4.2): their client consented to it, bound to the ceremony they are
     * standing in, and they are owed the telling even though they were not
     * asked. Everything else goes to the conversation's notice line.
     */
    fun told(notice: Told) {
        when (notice) {
            is Told.QuerySurfaced -> {
                val v = hex(notice.verifier).take(16)
                meet?.querySurfaced(v) ?: front.note("· a query about you was answered by $v…")
            }
            is Told.NomineesOutnumbered ->
                meet?.note("· witnesses: ${notice.mine} of mine, ${notice.theirs} of theirs")
            else -> front.note("· $notice")
        }
    }

    /** End the ceremony in progress. */
    fun stopMeet(reason: String) {
        meet?.stop(reason)
        synchronized(lock) { meet = null }
    }

    /** The one peer this device was provisioned to talk to, or null while
     *  unprovisioned: what a conversation screen opens onto. */
    fun peerKey(): String? = peerKey

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
        front.note("· provision stored. Restart the app for it to take effect.")
    }

    /**
     * Send application bytes to the provisioned peer.
     *
     * The bubble goes up at once as *sending*; the kernel's own send blocks
     * on the network, so it runs off the caller's thread and settles the
     * bubble to *sent* — accepted for carriage, never *delivered* — or to
     * *unsent* with the refusal's reason.
     */
    fun send(text: String) {
        val p = participant
        val to = peer
        val key = peerKey
        if (p == null || to == null || key == null) {
            front.note("· unprovisioned: nobody to send to")
            return
        }
        val id = front.outgoing(key, text)
        Thread {
            try {
                p.send(to, kindApplication(), text.toByteArray())
                front.settle(key, id, Front.Delivery.SENT, null)
            } catch (e: Refused.Reason) {
                front.settle(key, id, Front.Delivery.UNSENT, e.reason)
            }
        }.start()
    }

    /** Start the participant, attach where provisioned, and pump its events. */
    private fun bringUp(shell: AndroidShell) {
        val provision = shell.read("provision")?.let {
            try { JSONObject(String(it)) } catch (_: Exception) { null }
        }
        synchronized(lock) { provisioned = provision != null }
        front.setProvisioned(provision != null)
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
            front.setStatus("kernel refused: ${e.reason}")
            return
        }

        // the public half, for whoever must admit this device
        val material = p.material().joinToString("") { "%02x".format(it) }
        android.util.Log.i("fueros", "material $material")

        if (provision == null) {
            front.note("· unprovisioned. On the workstation:")
            front.note("    adb logcat -d -s fueros | grep material")
            front.note("    cargo run -p rhtn-ffi --features harness \\")
            front.note("      --bin payload-peer -- <that material>")
            front.note("· then hand its PROVISION line back as the `provision` extra.")
            return
        }

        try {
            val to = unhex(provision.getString("peer"))
            peer = to
            peerName = provision.optString("peer_name", "peer")
            peerKey = hex(to)
            front.peerKnown(peerKey!!, peerName)
            val node = unhex(provision.getString("node"))
            val a = p.attach(node, listOf(provision.getString("addr")), listOf(to))
            show(p, Status.Attached(node, a.primary))
        } catch (e: Refused.Reason) {
            front.setStatus("attach refused: ${e.reason}")
            return
        }
        while (true) {
            when (val e = p.nextEvent(2000UL)) {
                is Event.Payload -> {
                    val from = hex(e.from)
                    val who = if (from == peerKey) peerName else from.take(16)
                    front.incoming(from, who, String(e.bytes))
                }
                is Event.Connection -> show(p, e.v1)
                null -> {}
                else -> front.note("· $e")
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
     * running is this one, and it says it where its user can read it. The
     * direct-path chip is updated here too — **status, never a choice**: the
     * path order is fixed (design §12.6.3).
     */
    private fun show(p: Participant, s: Status) {
        val key = p.presentedKey().joinToString("") { "%02x".format(it) }.take(16)
        val line = when (s) {
            is Status.Detached -> "detached"
            is Status.Attached -> if (s.primary) "attached, primary" else "attached"
            is Status.Reconnecting -> "reconnecting…"
            is Status.Lost -> "lost: no serving node reachable"
        }
        // the kernel says which; the words are this shell's
        val construction = when (p.payloadConstruction()) {
            Construction.DOUBLE_RATCHET -> "the Double Ratchet (the floor)"
            Construction.TRIPLE_RATCHET -> "the Triple Ratchet"
        }
        front.setStatus("$line · presents $key…\npayload: $construction")
        val to = peer
        val k = peerKey
        if (to != null && k != null) {
            front.directPath(k, p.directTo(to))
        }
    }

    private fun unhex(s: String): ByteArray =
        ByteArray(s.length / 2) { ((s[2 * it].digitToInt(16) shl 4) + s[2 * it + 1].digitToInt(16)).toByte() }

    private fun hex(b: ByteArray): String = b.joinToString("") { "%02x".format(it) }

    /**
     * The device's own seeds, minted once.
     *
     * **Under the shell's own custody and never around it**: these are the
     * whole of the identity, and they never cross the kernel's storage
     * seam, so the kernel's sealing does not reach them — they are
     * Keystore-wrapped here, exactly as the storage key itself is
     * (`light-client-requirements.md` §9).
     */
    private fun mintedSeeds(shell: AndroidShell): ByteArray {
        shell.unseal("seeds")?.let { if (it.size == 64) return it }
        val s = ByteArray(64).also { SecureRandom().nextBytes(it) }
        check(shell.seal("seeds", s)) { "the device's own storage refused the seeds" }
        return s
    }
}
