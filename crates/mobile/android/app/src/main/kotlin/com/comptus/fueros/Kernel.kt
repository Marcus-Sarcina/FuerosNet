package com.comptus.fueros

import android.content.Context
import java.security.SecureRandom
import org.json.JSONObject
import uniffi.rhtn_ffi.Answer
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

    /** Whether the counterparty's contribution has been read off their
     *  screen, which is what decides the second QR from the first. */
    @Volatile private var opticalTaken = false

    /** The radio and the carriage over it, once the meeting opens one. */
    @Volatile private var ble: BleBearer? = null
    @Volatile private var carriage: Carriage? = null

    /** The application's context, kept for the radio the bearer needs. */
    @Volatile private var appContext: Context? = null

    /** Bind a screen: it is rendered against the state so far, then on
     *  every change. */
    fun bind(u: Front.Ui) = front.bind(u)

    fun unbind(u: Front.Ui) = front.unbind(u)

    fun meet(): Meet? = meet

    /**
     * Begin a ceremony with the one provisioned peer, meeting or adopting
     * as chosen. The kernel prepares the local half; the optical handshake
     * and the bearer that carries the intent are specified now
     * (`wire-format.md` §14.3) and carried from here: the two QRs on the
     * cameras and the intent over a Bluetooth LE bearer. A ceremony already
     * live is returned as-is.
     */
    fun startMeet(kind: Meet.Kind, role: Meet.Role): Meet? {
        val p = participant ?: return null
        val to = peer ?: return null
        val key = peerKey ?: return null
        synchronized(lock) {
            meet?.let { return it }
            meet = Meet(key, peerName, kind, role)
        }
        val m = meet!!
        Thread {
            try {
                // witnesses are nominated from the counterparty's
                // neighbourhood (design §7.1); with no horizon yet the
                // nomination is empty and the ceremony is that much weaker,
                // which the record carries honestly rather than hiding
                p.begin(to, listOf(), role == Meet.Role.INITIATOR)
                m.note("ceremony open; the two codes are ready to cross.")
            } catch (e: Refused.Reason) {
                m.stop("begin refused: ${e.reason}")
            }
        }.start()
        return m
    }

    // ---- the bootstrap, which is the shell's own object ----------------

    /**
     * **The bootstrap QR's payload** (D1a): this device's identifier and
     * the kind of transaction, and nothing of the ceremony's own anchor.
     *
     * **This one is not a §14.3 object and the shell owns it.** §14.3 fixes
     * the ANCHORED exchange, which is D2's; the bootstrap carries only what
     * design §7.1.1 needs to start a conversation between two people who
     * have not yet exchanged a contribution, so there is nothing here for
     * the kernel to check and nothing for it to encode. The shell's own
     * framing, versioned so a later one can differ.
     */
    fun bootstrap(): ByteArray? {
        val m = meet ?: return null
        val me = participant?.me() ?: return null
        val kind = when (m.adopt) {
            Meet.Adopt.NONE -> 0
            Meet.Adopt.THEM_UNDER_ME -> 1
            Meet.Adopt.ME_UNDER_THEM -> 2
        }
        val backup = if (m.kind.askBackup) 1 else 0
        return byteArrayOf(1, kind.toByte(), backup.toByte()) + me
    }

    /**
     * The counterparty's bootstrap. Null where it was taken; a reason
     * otherwise.
     *
     * **The kind it carries is the initiator's choice and the responder's
     * to refuse**, which is what the brief after this is for — so this
     * takes the choice without agreeing to it.
     */
    fun takeBootstrap(bytes: ByteArray): String? {
        if (bytes.size != 35 || bytes[0] != 1.toByte()) {
            return "that code is not a meeting invitation this build knows"
        }
        val m = meet ?: return "no meeting is open here"
        val theirs = bytes.copyOfRange(3, 35)
        if (hex(theirs) != m.counterpartyKey) {
            return "that code is someone else's, not ${m.counterpartyName}'s"
        }
        m.theirAdopt(
            when (bytes[1].toInt()) {
                1 -> Meet.Adopt.THEM_UNDER_ME
                2 -> Meet.Adopt.ME_UNDER_THEM
                else -> Meet.Adopt.NONE
            },
        )
        return null
    }

    // ---- the optical exchange, which is the kernel's ------------------

    /**
     * What to show on the screen at D2: the contribution first, then the
     * meeting id once the counterparty's contribution is in
     * (`wire-format.md` §14.3.1's two steps, in order).
     *
     * **The kernel decides which**, because only it knows whether it has
     * read the other side's contribution yet.
     */
    fun optical(): ByteArray? {
        val p = participant ?: return null
        return try {
            if (opticalTaken) p.transcriptConfirm() else p.opticalContribution()
        } catch (e: Refused.Reason) {
            null
        }
    }

    /**
     * Bytes the selfie camera read. Null where they were taken, a reason
     * otherwise — and a reason here stops the ceremony, because every
     * refusal at this step is either a bearer disagreeing with a screen or
     * a party who is not the one in front of you.
     */
    fun takeOptical(bytes: ByteArray): String? {
        val p = participant ?: return "the kernel is not running"
        val m = meet ?: return "no meeting is open here"
        return try {
            if (!opticalTaken) {
                p.takeOptical(bytes)
                opticalTaken = true
                m.note("their contribution is in; showing the meeting id.")
                null
            } else {
                val id = p.takeTranscript(bytes)
                m.note("the two meeting ids agree: ${hex(id).take(16)}…")
                // D3 can now run: the tap carries this agreed ceremony-id,
                // and the reader/card side follows the bootstrap's own
                // asymmetry — the party that showed the bootstrap reads.
                ProximityChannels.ceremony(id, m.role == Meet.Role.INITIATOR, true)
                // THE BEARER'S TURN. The optical step is what the integrity
                // rests on (§14.3.1); from here a radio carries the bulk,
                // and everything it carries is checked against what the
                // screens showed.
                carryIntent(m)
                null
            }
        } catch (e: Refused.Reason) {
            e.reason
        }
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
     * A query reaches its verifier on the end-to-end path once the
     * counterparty has consented to it (`wire-format.md` §5.6): the kernel
     * carries it, direct or through the serving nodes, and the answer lands
     * here as [Event.Answered]. What this build does not carry is the
     * consent exchange itself — the query to the counterparty's device and
     * the consent back — which is the ceremony's own local conversation
     * over a bearer the shell does not yet have. So the queries are
     * prepared and the selection is real; each waits on a consent that
     * cannot yet arrive.
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
                    m.note("${picked.size} query/queries prepared. Each waits on the")
                    m.note("counterparty's consent, which crosses the local bearer")
                    m.note("this build does not carry; consented, it is sent.")
                }
            } catch (e: Refused.Reason) {
                m.note("selection refused: ${e.reason}")
            }
        }.start()
    }

    /** `wire-format.md` §5.5's `selection_basis`, as the shell's own words
     *  render it; an unknown value is the discretionary tier, which claims
     *  the least. */
    private fun verdictOf(a: Answer): Meet.Verdict = when (a) {
        Answer.MATCH -> Meet.Verdict.MATCH
        Answer.NO_MATCH -> Meet.Verdict.NO_MATCH
        Answer.INCONCLUSIVE -> Meet.Verdict.INCONCLUSIVE
        Answer.UNAVAILABLE -> Meet.Verdict.UNAVAILABLE
    }

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
        synchronized(lock) {
            meet = null
            opticalTaken = false
            ble?.close()
            ble = null
            carriage = null
        }
        // the hardware goes back with the meeting: a camera held behind a
        // dead ceremony and a channel bound to a stale id are both the kind
        // of thing nobody consented to
        ProximityChannels.ceremony(null, false, false)
        shell?.endCapture()
    }

    /**
     * **The bearer's load** (`wire-format.md` §14.3.2): this device's
     * intent carriage out, the counterparty's in, over the best bearer the
     * two share.
     *
     * The radio is opened here rather than earlier because there is nothing
     * to carry until the anchor is agreed — and because a radio advertising
     * through a meeting nobody accepted is a beacon the person did not ask
     * for.
     *
     * **Who advertises and who scans carries no meaning.** §14.3.1 leaves
     * the bearer to the shell, so this uses the bootstrap's own asymmetry:
     * the party that showed the QR offers, the party that read it seeks.
     */
    private fun carryIntent(m: Meet) {
        val p = participant ?: return
        val to = peer ?: return
        val radio = BleBearer(appContext ?: return)
        // ONE inbound handler for the whole conversation. Each phase is
        // apart in the carriage (`Carriage.Phase`), so a packet arriving
        // is just dropped into its phase and the step waiting on that phase
        // is attempted — a late proximity packet cannot complete the intent
        // and a replayed intent packet cannot complete the capture.
        val inbound = BleBearer.Inbound { packet ->
            val live = carriage ?: return@Inbound
            live.packet(packet)?.let { why -> m.note("a packet was refused: $why") }
            drainBearer(p, to, m)
        }
        val why = when (m.role) {
            Meet.Role.INITIATOR -> radio.offer(inbound)
            Meet.Role.RESPONDER -> radio.seek(inbound)
        }
        if (why != null) {
            // §14.3.1's last resort is a network fetch, which this build
            // does not carry: said plainly rather than left to look like a
            // protocol failure
            m.note("no bearer: $why.")
            m.note("the network fetch §14.3.1 allows last is not built,")
            m.note("so the intent cannot cross from here.")
            return
        }
        synchronized(lock) {
            ble = radio
            carriage = Carriage(radio.link())
        }
        Thread {
            try {
                val mine = p.intentCarriage()
                if (carriage?.send(mine, Carriage.Phase.INTENT) == true) {
                    m.note("intent sent: ${mine.size} message(s) over the radio.")
                } else {
                    m.note("the radio would not take the intent; nothing was sent.")
                }
            } catch (e: Refused.Reason) {
                m.stop("the intent could not be built: ${e.reason}")
            }
        }.start()
    }

    /**
     * Take whatever phase has arrived whole, for the step the ceremony is
     * on. **Idempotent and order-free**: a phase already taken is not taken
     * twice (the step has moved on), and a phase not yet whole is left for
     * the next packet. This is called after every inbound packet and from
     * the step drivers, so a set that completed before its step was reached
     * is taken the moment it is.
     */
    private fun drainBearer(p: Participant, to: ByteArray, m: Meet): Unit = synchronized(bearerLock) {
        // SERIALIZED, because two threads reach here: the BLE inbound
        // callback when the counterparty's packets land, and the step
        // driver (runProximity/runCapture) right after it sends. received()
        // is non-destructive, so without this both could read one completed
        // phase, both take it, and the second advance() would throw on a
        // callback thread (step has already moved). The lock makes the
        // second entry see the advanced step and do nothing.
        val live = carriage ?: return
        when (m.step()) {
            Meet.Step.OPTICAL -> live.received(Carriage.Phase.INTENT)?.let { set ->
                try {
                    val taken = p.takeIntentCarriage(to, set)
                    m.note("their intent is in, with $taken continuation(s).")
                    m.opticalDone()
                } catch (e: Refused.Reason) {
                    // every refusal here is the kernel catching a bearer
                    // that disagrees with the screens (§14.3.2), which is
                    // the one thing the anchor is for
                    m.stop("the carried intent was refused: ${e.reason}")
                }
            }
            Meet.Step.PROXIMITY -> live.received(Carriage.Phase.PROXIMITY)?.let { set ->
                set.firstOrNull()?.let { bytes ->
                    try {
                        p.takeProximity(bytes)
                        m.note("their channel outcomes are in.")
                        m.proximityDone()
                    } catch (e: Refused.Reason) {
                        m.stop("the carried outcomes were refused: ${e.reason}")
                    }
                }
            }
            Meet.Step.CAPTURE -> live.received(Carriage.Phase.CAPTURE_KEY)?.let { set ->
                set.firstOrNull()?.let { bytes ->
                    var theirKey: ByteArray? = null
                    try {
                        // the carried `CaptureKeyHandover` is checked
                        // against this ceremony's id in the kernel, and
                        // their key is what comes back. It seals MY
                        // capture of them: the capture ran on this
                        // device's camera at D4 and is sealed beneath,
                        // silently (design §7.5.2.6)
                        theirKey = p.takeCaptureKeyCarriage(bytes)
                        p.capture(theirKey)
                        m.note("captures sealed; the meeting can be proposed.")
                        m.captureDone()
                    } catch (e: Refused.Reason) {
                        m.stop("the capture could not be sealed: ${e.reason}")
                    } finally {
                        // THE KEY IS LET GO HERE, sealed or not: it opens
                        // a likeness of a person, and the kernel wiped its
                        // own copies as the calls returned (design §7.5.2).
                        // Every byte of it this shell holds is zeroed, the
                        // bearer's assembly included
                        theirKey?.fill(0)
                        set.forEach { it.fill(0) }
                        live.discard(Carriage.Phase.CAPTURE_KEY)
                    }
                }
            }
            else -> {}
        }
    }

    /**
     * D3: run the channel ladder, send this device's outcomes, and take the
     * counterparty's. The outcomes cross as `ProximityOutcomes` (phase
     * PROXIMITY); the kernel weighs what they measured (design §1.3 item 4).
     */
    fun runProximity(m: Meet) {
        val p = participant ?: return
        val to = peer ?: return
        try {
            val mine = p.proximityCarriage()
            carriage?.send(listOf(mine), Carriage.Phase.PROXIMITY)
            m.note("channels run; the strongest that passed is recorded.")
            drainBearer(p, to, m)
        } catch (e: Refused.Reason) {
            m.stop("proximity: ${e.reason}")
        }
    }

    /**
     * D4: hand the counterparty the key that seals its captures of me, and
     * take theirs. The keys cross as `CaptureKeyHandover` (`wire-format.md`
     * §14.3.2), one each way as the one message of the CAPTURE_KEY phase,
     * anchored to the ceremony-id like the outcomes and checked against it
     * in the kernel when taken. The capture itself ran on this device's
     * camera as the kernel drove [AndroidShell.capture]; this is only the
     * keys crossing.
     */
    fun runCapture(m: Meet) {
        val p = participant ?: return
        val to = peer ?: return
        try {
            // the handover carries my key in clear: once the bearer has
            // had it, this shell keeps no copy, the packets it was cut
            // into included (design §7.5.2)
            val mine = p.captureKeyCarriage()
            try {
                carriage?.send(listOf(mine), Carriage.Phase.CAPTURE_KEY, wipe = true)
            } finally {
                mine.fill(0)
            }
            m.note("my capture key is sent; capturing the counterparty.")
            drainBearer(p, to, m)
        } catch (e: Refused.Reason) {
            m.stop("capture: ${e.reason}")
        }
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
            appContext = app
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
    @Volatile private var shell: AndroidShell? = null

    /** Serializes [drainBearer] against its two caller threads. */
    private val bearerLock = Any()

    private fun bringUp(shell: AndroidShell) {
        this.shell = shell
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
                // a verifier answered on the end-to-end path: the kernel
                // took it against the query it issued, and the screen
                // shows the verdict against the verifier it names
                is Event.Answered -> {
                    val v = e.answer
                    if (v != null) {
                        meet?.responded(hex(e.from), verdictOf(v))
                    } else {
                        front.note("· a response was refused: ${e.refused}")
                    }
                }
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
