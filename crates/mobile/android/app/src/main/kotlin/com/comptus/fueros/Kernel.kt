package com.comptus.fueros

import android.content.Context
import java.security.SecureRandom
import org.json.JSONObject
import uniffi.rhtn_ffi.Answer
import uniffi.rhtn_ffi.Construction
import uniffi.rhtn_ffi.Conversation
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
    /** Who this device nominates to witness, where the provision names
     *  anybody: a bench stand-in for the horizon a phone at genesis does
     *  not have (see [beginCeremony]). */
    @Volatile private var nominees: List<ByteArray> = listOf()
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
    /** How many conversation carriages this side has sent over the bearer
     *  in this ceremony: what picks each one's phase ([Carriage.Phase.conversation]). */
    private var sentCarriages = 0

    /**
     * **The one thread the bearer's work runs on, and never a GATT
     * callback's.**
     *
     * A packet arriving used to be assembled, handed to the kernel and
     * have the kernel's reply *sent* inline, on whatever thread the
     * Bluetooth stack delivered it on — which is a binder thread, and the
     * same binder thread that delivers write acknowledgements. `BleBearer`
     * serialises writes on their acknowledgement, so a send on that thread
     * waits for a permit only that thread can release. Run `shutter-1` of
     * 2026-10-07 shows it outright: acknowledgements arrive on
     * `binder:11774_4`, and `binder:11774_4` sat in the gate for the full
     * twenty-four seconds and then threw the ceremony's back-pointers
     * away [measured, 2026-10-07].
     *
     * One thread, not a pool: the phases are ordered, and two senders
     * racing for one permit is the other half of the same bug.
     */
    @Volatile private var bearerWork: java.util.concurrent.ExecutorService? = null

    /** The application's context, kept for the radio the bearer needs. */
    @Volatile private var appContext: Context? = null

    /** Bind a screen: it is rendered against the state so far, then on
     *  every change. */
    fun bind(u: Front.Ui) = front.bind(u)

    /** Unbind a screen: it is rendered no more. */
    fun unbind(u: Front.Ui) = front.unbind(u)

    /** The meeting in progress, where one is open. */
    fun meet(): Meet? = meet

    /** Whether the counterparty's contribution is in, which is what the
     *  screen's second QR at D2 is: the meeting id rather than ours. */
    fun opticalTaken(): Boolean = opticalTaken

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
        val retention = call("retention_years", { 2UL }) { p.retentionYears() }
        synchronized(lock) {
            meet?.let { return it }
            // nobody is named here: the codes say who is being met
            // (`wire-format.md` §14.3.1), the provision's peer being the
            // payload demo's and no part of a ceremony
            meet = Meet(kind = kind, role = role, retentionYears = retention.toLong()).also { m ->
                // the kernel's ceremony ends with the screen's: a Stop from
                // any live step abandons it, so nothing of the conversation
                // is answered after the person has left it
                m.onStopped = { call("abandon", { }) { p.abandon() } }
                // and begins with the brief's Accept, which is the one
                // question a ceremony asks the person, answered once
                m.onAccepted = { beginCeremony(p, m) }
            }
        }
        return meet
    }

    /**
     * Open the kernel's ceremony as the brief is accepted. The responder
     * names the initiator, whose bootstrap it read; the initiator names
     * nobody and learns who read its code from their first optical code.
     * Witnesses are nominated from the counterparty's neighbourhood
     * (design §7.1). This shell holds no horizon to nominate from yet, so
     * the nomination is empty and the ceremony is that much weaker, which
     * the record carries honestly — unless the provision named nominees,
     * which is the bench standing in for that horizon (`field-run.sh`): a
     * witness is reached over the network and need not be present
     * [author, 2026-10-05], so the laptop's instruments can be the ones.
     */
    private fun beginCeremony(p: Participant, m: Meet) {
        Thread {
            call("begin", { e -> m.stop("begin refused: ${e.reason}") }) {
                p.begin(m.counterpartyKey?.let { unhex(it) }, nominees, m.role == Meet.Role.INITIATOR)
                m.begunCeremony()
                m.note("ceremony open; the two codes are ready to cross.")
            }
        }.start()
    }

    /**
     * One call into the kernel, bracketed for the diagnostics
     * (`Robot/field-test-diagnostics.md`, section 3.6): `shell.call`
     * before, `shell.return` after with the elapsed milliseconds and,
     * where the kernel refused, its reason. The refusal is then handled
     * as it was: `onRefused` is the catch block, and returns what the
     * body does. In the releasable flavour the events go nowhere and the
     * try is what it was.
     */
    private inline fun <T> call(
        method: String,
        onRefused: (Refused.Reason) -> T,
        body: () -> T,
    ): T {
        Diag.event("shell.call", "method" to method)
        val started = Diag.ms()
        return try {
            val out = body()
            Diag.event("shell.return", "method" to method, "took_ms" to (Diag.ms() - started), "ok" to true)
            out
        } catch (e: Refused.Reason) {
            Diag.warn(
                "shell.return",
                "method" to method,
                "took_ms" to (Diag.ms() - started),
                "ok" to false,
                "refused" to Diag.scrub(e.reason),
            )
            onRefused(e)
        }
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
        // the code is who is being met: the first one names them
        m.counterparty(hex(bytes.copyOfRange(3, 35)))?.let { return it }
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
        return call(if (opticalTaken) "transcriptConfirm" else "opticalContribution", { null }) {
            if (opticalTaken) p.transcriptConfirm() else p.opticalContribution()
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
        if (!opticalTaken) {
            return call("takeOptical", { it.reason }) {
                // the kernel pins the key the code carries and names the
                // party by its hash; the flow learns who that is, or
                // refuses a code from anyone but the one it already knows
                val who = p.takeOptical(bytes)
                m.counterparty(hex(who))?.let { return@call it }
                opticalTaken = true
                m.note("their contribution is in; showing the meeting id.")
                null
            }
        }
        var refused: String? = null
        val id = call("takeTranscript", { refused = it.reason; null }) { p.takeTranscript(bytes) }
            ?: return refused ?: "the transcript was refused"
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
        return null
    }

    /**
     * **The conversation on the courier** (design §7.1 steps 6 to 8;
     * `wire-format.md` §7.10.1), from the capture on. The kernel takes the
     * four steps a participant takes and answers everything that arrives
     * in its own event loop; the flow decides *when*, from the kernel's
     * progress as [bringUp]'s loop polls it every tick. Every call crosses
     * [call], so the diagnostics bracket each.
     *
     * The verifier rows the screen shows are [Participant.selectVerifiers]'s,
     * asked for once here before `converseQueries` selects the same set
     * for itself: the selection is a sort over the pool and a fill, with
     * nothing random in it (`crates/client/src/selection.rs`), so the two
     * agree. **The selection is the kernel's and not a choice on a
     * screen**: the tiers are computed from what this device knows and the
     * pool from the records the counterparty handed over, so the screen
     * shows who was picked and why, never a list to choose from. Each
     * query reaches its verifier once the counterparty has consented to it
     * on the same path (`wire-format.md` §5.6), and the answer lands here
     * as [Event.Answered].
     */
    private fun courier(p: Participant): Meet.Courier = object : Meet.Courier {
        // each step's leg to the counterparty is a carriage for the
        // bearer, drained right after the step whether it went or was
        // refused: a refused propose that is `Waiting` has sent nothing,
        // and a step that went has left its carriage in the kernel
        override fun open(): String? = refusal("converseOpen") { p.converseOpen() }.also { sendCarriages(p) }
        override fun queries(): String? = refusal("converseQueries") { p.converseQueries() }.also { sendCarriages(p) }
        override fun gathered(): String? = refusal("converseGathered") { p.converseGathered() }.also { sendCarriages(p) }
        override fun propose(): String? = refusal("conversePropose") { p.conversePropose() }.also { sendCarriages(p) }
        override fun progress(): Meet.Progress? =
            call("progress", { null }) { p.progress() }?.let { pr ->
                Meet.Progress(
                    proposer = pr.proposer,
                    queriesOutstanding = pr.queriesOutstanding.toInt(),
                    attesting = pr.attesting.map { hex(it) },
                    declined = pr.declined.map { hex(it) },
                    backFrom = pr.backFrom.map { hex(it) },
                    theirResponses = pr.theirResponses,
                    proposed = pr.proposed,
                    signed = pr.signed.map { hex(it) },
                    refused = pr.refused.map { hex(it) },
                )
            }
    }

    /** One step of the conversation: null where it went, the kernel's
     *  reason where it refused. */
    private inline fun refusal(method: String, body: () -> Unit): String? =
        call(method, { it.reason }) { body(); null }

    /**
     * Enter the conversation as the captures seal: the nominees and the
     * verifier rows for the screen first, then the kernel's open and its
     * queries, off the thread the bearer's packet arrived on, since each
     * sends over the network. The flow opens once; a second entry is its
     * own no-op.
     */
    private fun startConversation(p: Participant, m: Meet) {
        Thread {
            call("nominees", { null }) { p.nominees() }?.let { n ->
                m.nominated(n.mine.map { hex(it) }, n.theirs.map { hex(it) })
            }
            val picked = call("selectVerifiers", { e -> m.note("selection refused: ${e.reason}"); null }) {
                p.selectVerifiers()
            }
            if (picked != null) {
                m.selected(picked.map { Meet.Chosen(hex(it.verifier), basisOf(it.basis)) })
                if (picked.isEmpty()) {
                    m.note("no verifier is required: the counterparty handed over")
                    m.note("no records, so wire-format §5.2 obliges none.")
                }
            }
            m.converse(courier(p))
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

    /** The kernel's move of the conversation, as the flow switches on it:
     *  the three it acts on, and the rest. */
    private fun turnOf(c: Conversation): Meet.Turn = when (c) {
        is Conversation.Finalized -> Meet.Turn.Finalized(hex(c.txid))
        is Conversation.Reviewed ->
            c.refused?.let { Meet.Turn.BodyRefused(it.code.toInt(), it.why) } ?: Meet.Turn.Other
        is Conversation.Signed ->
            c.refused?.let { Meet.Turn.SignerRefused(hex(c.signer), it.code.toInt(), it.why) } ?: Meet.Turn.Other
        else -> Meet.Turn.Other
    }

    /** What a conversation event refused, for the diagnostics: a signing
     *  refusal's code and words, or a stranger's message's reason. */
    private fun refusalIn(c: Conversation): String? = when (c) {
        is Conversation.Reviewed -> c.refused?.let { "${it.code}: ${it.why}" }
        is Conversation.Signed -> c.refused?.let { "${it.code}: ${it.why}" }
        is Conversation.Refused -> "kind ${c.kind}: ${c.why}"
        else -> null
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
        val work = synchronized(lock) {
            meet = null
            opticalTaken = false
            ble?.close()
            ble = null
            carriage = null
            bearerWork.also { bearerWork = null }
        }
        // the bearer's thread goes with the meeting; a send still in the
        // gate is not waited for, since the link it was for is closed
        work?.shutdownNow()
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
        // **off the stack's thread, always** ([bearerWork]): everything
        // below this point can send, and a send blocks on an
        // acknowledgement that a GATT callback thread is the only one able
        // to deliver
        val work = java.util.concurrent.Executors.newSingleThreadExecutor { r ->
            Thread(r, "bearer").apply { isDaemon = true }
        }
        synchronized(lock) { bearerWork = work }
        val inbound = BleBearer.Inbound { packet ->
            val live = carriage ?: return@Inbound
            if (work.isShutdown) return@Inbound
            try {
                work.execute {
                    if (carriage !== live) return@execute
                    live.packet(packet)?.let { why -> m.note("a packet was refused: $why") }
                    drainBearer(p, to, m)
                }
            } catch (e: java.util.concurrent.RejectedExecutionException) {
                // the meeting ended between the check and the submit
            }
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
            sentCarriages = 0
        }
        Thread {
            // THE LINK FIRST. offer and seek return as the radio starts,
            // not as a peer connects and subscribes, and a packet sent
            // before the subscription goes nowhere while reporting success
            // (BleBearer). So the intent waits, bounded, on the link coming
            // ready; a link that does not is a stopped meeting with a
            // reason, never an intent that silently did not cross.
            if (!awaitLink(radio, m, "intent")) return@Thread
            val mine = call("intentCarriage", { e ->
                m.stop("the intent could not be built: ${e.reason}")
                null
            }) { p.intentCarriage() } ?: return@Thread
            if (carriage?.send(mine, Carriage.Phase.INTENT) == true) {
                m.note("intent sent: ${mine.size} message(s) over the radio.")
                // proximity starts once theirs is in as well
                m.intentSent()
            } else {
                m.stop("the radio would not take the intent; nothing crossed")
            }
        }.start()
    }

    /** How long a phase waits on the Bluetooth link before the meeting
     *  stops: a scan, a connection, the MTU, discovery and the subscription,
     *  on two phones' stacks. */
    private const val LINK_READY_MS = 30_000L

    /**
     * Wait for the link `radio` presents to come ready, for `phase`, or
     * stop the meeting. Each phase waits here, not only the intent: a
     * disconnect between phases clears the subscription and the next phase
     * would otherwise send into the gap. A meeting stopped meanwhile ends
     * the wait without a second reason. The wait and its outcome are one
     * `ble` event each (`Robot/field-test-diagnostics.md`, section 3.6).
     */
    private fun awaitLink(radio: BleBearer, m: Meet, phase: String): Boolean {
        if (radio.ready()) return true
        val started = Diag.ms()
        val ready = Bearer.awaitReady(LINK_READY_MS) { radio.ready() || m.step() == Meet.Step.STOPPED }
        if (m.step() == Meet.Step.STOPPED) return false
        if (!ready) {
            Diag.warn("ble", "op" to "link", "phase" to phase, "state" to "timeout", "took_ms" to (Diag.ms() - started))
            m.stop("the Bluetooth link did not come ready within ${LINK_READY_MS / 1000} s, so the $phase could not cross")
            return false
        }
        Diag.event("ble", "op" to "link", "phase" to phase, "state" to "awaited", "took_ms" to (Diag.ms() - started))
        return true
    }

    /**
     * **The counterparty's leg of the conversation crosses the bearer**
     * (design §7.1; `wire-format.md` §14.3.2): kinds 9 to 18 for the
     * co-present counterparty are sealed under the local session and sit
     * in the kernel until drained, and no node carries them. Drained
     * after every step this side takes, after every carriage taken, and
     * after every message that arrived over the network, since each can
     * produce one; one message per phase, so the receiver can take each
     * as it lands.
     *
     * **A carriage the radio will not take ends the ceremony.** The
     * kernel holds no copy, so the message is gone, and the counterparty
     * is waiting on a phase that no longer exists anywhere — it cannot
     * know that, and it cannot recover. This was a note and nothing more
     * until run `aimed-1` of 2026-10-07, where one dropped phase left the
     * proposer at `VERIFIERS` indefinitely while both phones went on
     * polling; the step's own comment said it "waits until the person
     * stops the meeting", and that is not a failure mode to leave to the
     * person. Stopping says so on both screens and gives the hardware
     * back [author, 2026-10-07].
     */
    private fun sendCarriages(p: Participant) {
        val live = carriage ?: return
        val out = call("carriages", { listOf() }) { p.carriages() }
        for (bytes in out) {
            val phase = Carriage.Phase.conversation(sentCarriages++)
            if (!live.send(listOf(bytes), phase)) {
                stopMeet("a carriage of the conversation did not cross the bearer (phase $phase)")
                return
            }
        }
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
                // every refusal here is the kernel catching a bearer
                // that disagrees with the screens (§14.3.2), which is
                // the one thing the anchor is for
                call("takeIntentCarriage", { e -> m.stop("the carried intent was refused: ${e.reason}") }) {
                    val taken = p.takeIntentCarriage(to, set)
                    m.note("their intent is in, with $taken continuation(s).")
                    // proximity starts once ours has gone as well
                    m.intentReceived()
                }
            }
            Meet.Step.PROXIMITY -> live.received(Carriage.Phase.PROXIMITY)?.let { set ->
                set.firstOrNull()?.let { bytes ->
                    call("takeProximity", { e -> m.stop("the carried outcomes were refused: ${e.reason}") }) {
                        p.takeProximity(bytes)
                        m.note("their channel outcomes are in.")
                        m.proximityDone()
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
                        val sealed = { e: Refused.Reason ->
                            m.stop("the capture could not be sealed: ${e.reason}")
                        }
                        val k = call("takeCaptureKeyCarriage", { sealed(it); null }) {
                            p.takeCaptureKeyCarriage(bytes)
                        }
                        if (k != null) {
                            theirKey = k
                            call("capture", sealed) {
                                p.capture(k)
                                m.note("captures sealed; the conversation opens.")
                                m.captureDone()
                                // from here the courier carries the
                                // ceremony, and the kernel's loop drives
                                startConversation(p, m)
                            }
                        }
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
            // the conversation: every phase from CONVERSATION up that has
            // arrived whole is one message of the counterparty's, taken
            // and its phase discarded so the sender's count can wrap.
            // What a taken step owes the counterparty goes straight back
            Meet.Step.VERIFIERS, Meet.Step.REVIEW -> {
                var taken = 0
                for (phase in Carriage.Phase.CONVERSATION until Bearer.PHASES) {
                    val set = live.received(phase) ?: continue
                    for (bytes in set) {
                        call("takeCarriage", { e -> m.note("a carriage was refused: ${e.reason}") }) {
                            p.takeCarriage(bytes)
                        }
                        taken += 1
                    }
                    live.discard(phase)
                }
                if (taken > 0) sendCarriages(p)
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
        val radio = ble ?: return m.stop("no radio carries the channel outcomes")
        if (!awaitLink(radio, m, "proximity")) return
        val mine = call("proximityCarriage", { e -> m.stop("proximity: ${e.reason}"); null }) {
            p.proximityCarriage()
        } ?: return
        if (carriage?.send(listOf(mine), Carriage.Phase.PROXIMITY) != true) {
            return m.stop("the radio would not take the channel outcomes; nothing crossed")
        }
        m.note("channels run; the strongest that passed is recorded.")
        drainBearer(p, to, m)
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
        // the handover carries my key in clear: once the bearer has
        // had it, this shell keeps no copy, the packets it was cut
        // into included (design §7.5.2)
        val radio = ble ?: return m.stop("no radio carries the capture key")
        if (!awaitLink(radio, m, "capture key")) return
        val mine = call("captureKeyCarriage", { e -> m.stop("capture: ${e.reason}"); null }) {
            p.captureKeyCarriage()
        } ?: return
        val sent = try {
            carriage?.send(listOf(mine), Carriage.Phase.CAPTURE_KEY, wipe = true) == true
        } finally {
            mine.fill(0)
        }
        if (!sent) return m.stop("the radio would not take the capture key; nothing crossed")
        m.note("my capture key is sent; capturing the counterparty.")
        drainBearer(p, to, m)
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
            call("send", { e -> front.settle(key, id, Front.Delivery.UNSENT, e.reason) }) {
                p.send(to, kindApplication(), text.toByteArray())
                front.settle(key, id, Front.Delivery.SENT, null)
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
        val p = call("Participant.start", { e -> front.setStatus("kernel refused: ${e.reason}"); null }) {
            Participant.start(mintedSeeds(shell), known, platformOf(shell))
        } ?: return
        participant = p
        show(p, Status.Detached)

        val material = p.material().joinToString("") { "%02x".format(it) }

        if (provision == null) {
            // the material is this device's public identity, shown here
            // because nothing else exposes it: no flavour logs it and the
            // Report bundle carries identities as eight characters only
            front.note("· unprovisioned. This device's public key material:")
            front.note("    $material")
            front.note("· on the workstation, with that material copied off the screen:")
            front.note("    cargo run -p rhtn-ffi --features harness \\")
            front.note("      --bin payload-peer -- <that material>")
            front.note("· then hand its PROVISION line back as the `provision` extra.")
            return
        }

        // the retention this device declares (design §7.5.1), where the
        // provision names one; the kernel's own default otherwise
        val retention = provision.optLong("retention_years", 0L)
        if (retention > 0L) {
            call("set_retention_years", { e -> front.note("· retention refused: ${e.reason}") }) {
                p.setRetentionYears(retention.toULong())
            }
        }
        val to = unhex(provision.getString("peer"))
        peer = to
        peerName = provision.optString("peer_name", "peer")
        nominees = provision.optJSONArray("nominees")
            ?.let { a -> (0 until a.length()).map { unhex(a.getString(it)) } }
            ?: listOf()
        peerKey = hex(to)
        front.peerKnown(peerKey!!, peerName)
        val node = unhex(provision.getString("node"))
        val a = call("attach", { e -> front.setStatus("attach refused: ${e.reason}"); null }) {
            p.attach(node, listOf(provision.getString("addr")), listOf(to))
        } ?: return
        show(p, Status.Attached(node, a.primary))
        while (true) {
            val e = p.nextEvent(2000UL)
            // the kind of thing that arrived and who from, as eight hex
            // characters; a payload's bytes are counted, never shown
            if (e != null && Diag.enabled()) {
                Diag.event(
                    "shell.event",
                    "kind" to e::class.simpleName,
                    "from" to when (e) {
                        is Event.Payload -> Diag.id8(e.from)
                        is Event.Answered -> Diag.id8(e.from)
                        is Event.ResponseCopy -> Diag.id8(e.from)
                        is Event.Late -> Diag.id8(e.from)
                        is Event.Conversed -> Diag.id8(e.from)
                        else -> null
                    },
                    "bytes" to (e as? Event.Payload)?.bytes?.size,
                    "refused" to when (e) {
                        is Event.Answered -> e.refused
                        is Event.ResponseCopy -> e.refused
                        is Event.Late -> e.refused
                        is Event.Conversed -> refusalIn(e.step)
                        else -> null
                    },
                )
            }
            when (e) {
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
                // a message of the ceremony's conversation, in the kernel's
                // words, and the record where it finalized: the flow reads
                // both; outside a meeting it is this device witnessing
                // somebody else's, which the notice line carries
                is Event.Conversed -> {
                    val m = meet
                    if (m != null) m.conversed(e.what, turnOf(e.step)) else front.note("· ${e.what}")
                    // a step taken on a message from a witness can owe the
                    // counterparty one: the last signature in, the record out
                    if (m != null) sendCarriages(p)
                }
                null -> {}
                else -> front.note("· $e")
            }
            // THE FLOW'S CLOCK. The conversation is polled every tick while
            // it is open, after whatever arrived: this is where the
            // proposer learns its queries are answered and proposes, where
            // the responder hands over, and where the body's arrival moves
            // to review. The poll is nothing outside those steps.
            meet?.let { m ->
                // a conversation phase that landed before the flow reached
                // the conversation is taken on the tick, as is one whose
                // packet arrived while the step driver held the lock
                val t = m.step()
                if (t == Meet.Step.VERIFIERS || t == Meet.Step.REVIEW) {
                    peer?.let { drainBearer(p, it, m) }
                }
                if (m.opened()) m.poll(courier(p))
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
