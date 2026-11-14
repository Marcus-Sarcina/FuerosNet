package com.comptus.fueros

import android.content.Context
import org.json.JSONObject
import uniffi.rhtn_ffi.Answer
import uniffi.rhtn_ffi.Construction
import uniffi.rhtn_ffi.Conversation
import uniffi.rhtn_ffi.Event
import uniffi.rhtn_ffi.Participant
import uniffi.rhtn_ffi.Refused
import uniffi.rhtn_model.Connection
import uniffi.rhtn_model.Contact
import uniffi.rhtn_model.Cost
import uniffi.rhtn_model.How
import uniffi.rhtn_model.Instance
import uniffi.rhtn_model.Line
import uniffi.rhtn_model.Meeting
import uniffi.rhtn_model.Resource
import uniffi.rhtn_model.Subnet
import uniffi.rhtn_ffi.Told
import uniffi.rhtn_ffi.TraceStep
import uniffi.rhtn_ffi.kindApplication

/**
 * **The ceremony, as the shell runs it** [restructure step 4, 2026-11-14]:
 * one meeting at a time on this device, held as the identity named at its
 * start ([Meet.by]); the optical exchange, the bootstrap, the bearer, the
 * conversation's turns, and the adoption and peering a meeting carries.
 * The per-identity event loop hands a meeting's events here ([took]) and
 * ticks its clock ([tick]); nothing here reads the focus.
 */
object Ceremony {
    private val lock = Any()

    /**
     * **One loop's instrumentation clock** — step 2 of the browser model
     * [author, 2026-11-11]: these were three fields of the kernel, which
     * with a loop per identity would have each loop's heartbeat
     * suppressing the others' on the shared clock, so a park in one
     * identity's `nextEvent` could hide behind another's turns. One of
     * these per [runLoop], and the fields the loop reads are its own.
     */
    internal class TurnDiag {
        var lastTurnState = ""
        var lastTurnAt = 0L
        var lastEntryAt = 0L
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
    /** How often the turn heartbeat repeats an unchanged state, so that a
     *  loop which is alive but idle is distinguishable from one that has
     *  stopped. Five seconds: frequent enough that a tester waiting on a
     *  screen sees it move, rare enough not to be the chatter a client
     *  review already objected to [2026-10-11]. */
    private val turnDiagMs = 5_000L

    /** The one thread the trace goes out on, in order (see [opticalTrace]). */
    private val traceWork: java.util.concurrent.ExecutorService =
        java.util.concurrent.Executors.newSingleThreadExecutor { r ->
            Thread(r, "fueros-trace").apply { isDaemon = true }
        }

    /** Whether [drainBearerLocked] took a conversation carriage that the
     *  flow has not yet been polled for. */
    private val carriedPending = java.util.concurrent.atomic.AtomicBoolean(false)

    /** Serializes [drainBearer] against its two caller threads. */
    private val bearerLock = Any()

    /**
     * **The one thread the bearer's work runs on, and never a GATT
     * callback's.** A packet is delivered on a binder thread, the same one
     * that delivers write acknowledgements; `BleBearer` serialises writes
     * on their acknowledgement, so a send on that thread waits for a
     * permit only that thread can release — which hung a ceremony for the
     * full twenty-four seconds of the gate [measured, 2026-10-07]. One
     * thread and not a pool: the phases are ordered, and two senders
     * racing for one permit is the other half of the same bug.
     */
    @Volatile private var bearerWork: java.util.concurrent.ExecutorService? = null

    /** Whether this meeting has already said the trace could not go. */
    private val traceRefusedSaid = java.util.concurrent.atomic.AtomicBoolean(false)

    private val earlyPeering = mutableListOf<Pair<String, ByteArray>>()

    /** Frames of an adoption or a peering that arrived before this side began its own (see the loop). */
    private val earlyAdoption = mutableListOf<Pair<String, ByteArray>>()

    private val sealLock = Any()

    /** The conversation is entered once per meeting, as the window opens. */
    private val conversationStarted = java.util.concurrent.atomic.AtomicBoolean(false)

    /** Whether this device's hold has run its window: the seal waits on
     *  this and on [theirCaptureKey], whichever lands last. */
    private val captureWindowDone = java.util.concurrent.atomic.AtomicBoolean(false)

    /** The counterparty's capture key, from the bearer until the seal takes
     *  it; wiped the moment it is used or the meeting ends. */
    @Volatile internal var theirCaptureKey: ByteArray? = null

    /** How many conversation carriages this side has sent over the bearer
     *  in this ceremony: what picks each one's phase ([Carriage.Phase.conversation]). */
    private val sentCarriages = java.util.concurrent.atomic.AtomicInteger(0)

    @Volatile internal var carriage: Carriage? = null

    /** The radio and the carriage over it, once the meeting opens one. */
    @Volatile internal var ble: BleBearer? = null

    /** Whether the transcript has been taken for the meeting open: a
     *  second is the screen repeating itself, and starts no second bearer. */
    @Volatile internal var transcriptTaken = false

    /** Whether the counterparty's contribution has been read off their
     *  screen, which is what decides the second QR from the first. */
    @Volatile internal var opticalTaken = false

    @Volatile internal var peering: Peering? = null

    /** The adoption after the record (D7), or null where the meeting
     *  carried none. Outlives the ceremony: the record closes the
     *  ceremony and the adoption rests on it. */
    @Volatile internal var adoption: Adoption? = null

    /** The ceremony in progress, or null. Process-scoped like the
     *  connection: a font change mid-ceremony must not lose it. */
    @Volatile internal var meet: Meet? = null

    /** How long a phase waits on the Bluetooth link before the meeting
     *  stops: a scan, a connection, the MTU, discovery and the subscription,
     *  on two phones' stacks. */
    private const val LINK_READY_MS = 30_000L

    /** One step of the conversation: null where it went, the kernel's
     *  reason where it refused. */
    private inline fun refusal(method: String, body: () -> Unit): String? =
        Gateway.call(method, { it.reason }) { body(); null }

    /** One kernel call whose refusal is the adoption's to carry. */
    private inline fun <T> result(method: String, body: () -> T): Adoption.Result<T> =
        Gateway.call(method, { Adoption.Result.Refused(it.reason) }) { Adoption.Result.Ok(body()) }

    /** One kernel call whose refusal is the peering's to carry. */
    private inline fun <T> peered(method: String, body: () -> T): Peering.Result<T> =
        Gateway.call(method, { Peering.Result.Refused(it.reason) }) { Peering.Result.Ok(body()) }

    /**
     * Begin a ceremony, meeting or adopting as chosen, with whoever the
     * codes turn out to name (`wire-format.md` §14.3). A ceremony already
     * live is returned as-is.
     */
    fun startMeet(by: String, kind: Meet.Kind, role: Meet.Role): Meet? {
        val p = Gateway.loaded[by] ?: return null
        val retention = Gateway.call("retention_years", { 2UL }) { p.retentionYears() }
        synchronized(lock) {
            // a meeting still under way is the one to return to; one that
            // ended either way is residue, and the next starts clean
            meet?.let { if (it.step() != Meet.Step.DONE && it.step() != Meet.Step.STOPPED) return it }
            // **and carries nothing of the last** [bench, 2026-10-30: T5's
            // Done screen reported T4's peering, the object never having
            // been cleared]: the adoption and the peering are this
            // meeting's or nobody's
            adoption = null
            peering = null
            // nobody is named here: the codes say who is being met
            // (`wire-format.md` §14.3.1), the provision's Attachments.peer being the
            // payload demo's and no part of a ceremony
            resetWindow()
            meet = Meet(by = by, kind = kind, role = role, retentionYears = retention.toLong()).also { m ->
                // the kernel's ceremony ends with the screen's: a Stop from
                // any live step abandons it, so nothing of the conversation
                // is answered after the person has left it
                m.onStopped = { Gateway.call("abandon", { }) { p.abandon() } }
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
     * (design §7.1). This Gateway.shell holds no horizon to nominate from yet, so
     * the nomination is empty and the ceremony is that much weaker, which
     * the record carries honestly — unless the provision named Attachments.nominees,
     * which is the bench standing in for that horizon (`field-run.sh`): a
     * witness is reached over the network and need not be present
     * [author, 2026-10-05], so the laptop's instruments can be the ones.
     */
    private fun beginCeremony(p: Participant, m: Meet) {
        Thread {
            Gateway.call("begin", { e -> m.stop("begin refused: ${e.reason}") }) {
                p.begin(m.counterpartyKey?.let { Gateway.unhex(it) }, Attachments.nominees, m.role == Meet.Role.INITIATOR)
                m.begunCeremony()
                m.note("ceremony open; the two codes are ready to cross.")
            }
            // **the Attachments.nominees are engaged with the intent, before the code
            // is shown** [author, 2026-10-19]: they observe from before
            // the counterparty has accepted anything, which is their
            // value — a witness shown only finished records adds no
            // friction to a replay (design §7.1). Retried while their
            // session material is fetched, and a nominee still unreached
            // when the ceremony-id fixes is one that does not appear
            engageNominees(p, m)
        }.start()
    }

    /**
     * **Engage this side's Attachments.nominees** (kind 20), retried while the
     * bindings land: the first attempt on a nominee whose session
     * material is not held asks for it, so a transient absence is not an
     * unavailable witness (`light-client-requirements.md` §1.1). The
     * attempts stop once every nominee is reached, the window opens, or
     * the tries run out — the kernel closes the set when the ceremony-id
     * fixes, and reports what it reached.
     */
    private fun engageNominees(p: Participant, m: Meet) {
        val c = courier(m.by, p)
        for (attempt in 0 until Engagement.TRIES) {
            if (m.step() == Meet.Step.STOPPED) return
            c.engage()?.let { why ->
                Diag.warn("witness.engage", "why" to why)
                return
            }
            val reached = Gateway.call("nomineesEngaged", { null }) { p.nomineesEngaged() } ?: return
            // **the decision is [Engagement]'s and the asking is this
            // function's** [author, 2026-11-05]: the retry and the words
            // were in here, where nothing could reach them without a live
            // `Participant`, so two of PRD-17's clauses had no test
            when (val next = Engagement.next(reached.engaged, reached.nominated, attempt)) {
                is Engagement.Next.Stop -> return
                is Engagement.Next.Say -> {
                    m.note(next.note)
                    return
                }
                is Engagement.Next.Again -> Thread.sleep(Engagement.WAIT_MS)
            }
        }
    }

    /**
     * **Start the adoption the meeting carried**, once the record is held
     * (D7): the responder states the direction the two people's choices
     * came to and the initiator is told it, after which the party being
     * adopted offers its series and the patron proposes.
     *
     * Called where the record lands and again where a person resolves an
     * opposition from the screen (PRD-05); the second call does nothing,
     * an adoption being begun once.
     *
     * **Nothing happens for a meeting that carried no adoption**, and
     * nothing happens while an opposition is unresolved: the question is
     * the two people's and the screen is asking it.
     */
    fun adoptionBegins(by: String) {
        val p = Gateway.loaded[by] ?: return
        val m = meet ?: return
        val txid = m.recordTxid() ?: return
        val them = m.counterpartyKey ?: return
        if (adoption != null) return
        val settled = if (m.role == Meet.Role.RESPONDER) m.settled() else null
        // **a meeting that was never going to move authority says
        // nothing**: the inviter's code asked for none, so the responder
        // has nothing to state and the initiator nothing to be told
        if (settled == Meet.Adopt.NONE) return
        if (m.role == Meet.Role.INITIATOR && m.adopt == Meet.Adopt.NONE) {
            // the initiator's own choice is the transaction's kind; where
            // it asked for no adoption the responder has nothing to settle
            return
        }
        val a = Adoption(
            role = m.role,
            counterparty = them,
            counterpartyName = m.counterpartyName,
            presence = txid,
            kern = adoptionKern(p),
            post = { bytes ->
                Gateway.call("send", { e -> Diag.warn("adopt.send", "refused" to e.reason); false }) {
                    p.send(Gateway.unhex(them), Adoption.KIND, bytes)
                    true
                }
            },
        )
        a.onChanged = { m.rendered() }
        adoption = a
        m.rendered()
        // what arrived before this side began goes in now, in order
        val early = synchronized(lock) { earlyAdoption.filter { it.first == them }.also { earlyAdoption.clear() } }
        early.forEach { (_, bytes) -> a.frame(bytes) }
        // off the caller's thread: every leg of this sends
        // the responder states the direction and which subnet, in words,
        // in the other side's frame: "a new subnet under <me>", not "under
        // your own key" [bench, 2026-10-15]
        // **by the name this device goes by**, where it has one [bench,
        // 2026-10-30: "a new subnet under bfcb69c0" read on the other
        // phone while its rail said GXV]: the subnet's own name for it,
        // else the default, else the keyhash's first characters
        val label = m.labelForCounterparty(
            Gateway.subnets(by).firstNotNullOfOrNull { it.ownAlias?.takeIf { a -> a.isNotBlank() } }
                ?: Gateway.defaultAlias(by)
                ?: Gateway.hex(p.me()).take(8),
        )
        Thread { if (settled != null) a.settle(settled, label) else a.begin() }.start()
    }

    /** The kernel calls an [Adoption] makes, bracketed like every other. */
    private fun adoptionKern(p: Participant): Adoption.Kern = object : Adoption.Kern {
        override fun freshSeries(): Int? =
            Gateway.call("freshSeries", { null }) { p.freshSeries((1u..16u).toList()) }?.toInt()

        override fun backPointers(): List<ByteArray> =
            Gateway.call("backPointers", { emptyList() }) { p.backPointers() }

        override fun carrying(): String? =
            meet?.carrying()?.id?.takeIf { it.length == 64 }

        override fun position(): String = Gateway.hex(p.position())

        override fun propose(
            node: String,
            presence: String,
            series: Int,
            nodeBack: List<ByteArray>,
            carrying: String?,
        ): Adoption.Result<ByteArray> = result("proposeAdoption") {
            // into the subnet this device chose as patron: one it holds a
            // position in, or none for a new one under its own key; and
            // the scope the other brings, their claim, copied
            val into = meet?.scope()?.id?.takeIf { it.length == 64 }?.let { Gateway.unhex(it) }
            p.proposeAdoption(into, Gateway.unhex(node), Gateway.unhex(presence), series.toUInt(), nodeBack, carrying?.let { Gateway.unhex(it) })
        }

        override fun reviewAndSign(body: ByteArray): Adoption.Result<ByteArray> =
            result("reviewAndSignAdoption") { p.reviewAndSignAdoption(body) }

        override fun envelope(
            body: ByteArray,
            entries: List<Pair<String, ByteArray>>,
        ): Adoption.Result<ByteArray> = result("adoptionEnvelope") {
            val mine = p.signBody(body)
            val all = entries.map { (who, e) -> uniffi.rhtn_ffi.SignedEntries(Gateway.unhex(who), e) } +
                uniffi.rhtn_ffi.SignedEntries(p.me(), mine)
            uniffi.rhtn_ffi.adoptionEnvelope(body, all)
        }

        override fun take(envelope: ByteArray): Adoption.Result<String> =
            result("takeAdoption") { Gateway.hex(p.takeAdoption(envelope)) }

        override fun positionIn(anchor: String): String? =
            Gateway.call("positionIn", { null }) { p.positionIn(Gateway.unhex(anchor)) }?.let { Gateway.hex(it) }
    }

    /**
     * **Start the peering the meeting carried**, once the record is held
     * (A3; design §6.3). A peering has no direction to settle, so this
     * needs nothing from either person beyond the decision to make one:
     * the lower keyhash builds the body and the other sends what its
     * archive chains to.
     *
     * Called where the record lands, and a second call does nothing — a
     * peering is begun once. **Nothing happens for a meeting that asked
     * for none**: the checkbox at the invitation is where the two chose
     * it [author, 2026-10-19], the responder reading the initiator's
     * choice off the bootstrap. **A peering and an adoption are not
     * alternatives**: a meeting can carry either, both or neither, and
     * nothing here consults the adoption's state.
     */
    fun peeringBegins(by: String) {
        val p = Gateway.loaded[by] ?: return
        val m = meet ?: return
        if (!m.peeringAsked()) return
        val txid = m.recordTxid() ?: return
        val them = m.counterpartyKey ?: return
        if (peering != null) return
        val pr = Peering(
            me = Gateway.hex(p.me()),
            counterparty = them,
            counterpartyName = m.counterpartyName,
            presence = txid,
            kern = peeringKern(p),
            post = { bytes ->
                Gateway.call("send", { e -> Diag.warn("Attachments.peer.send", "refused" to e.reason); false }) {
                    p.send(Gateway.unhex(them), Peering.KIND, bytes)
                    true
                }
            },
        )
        pr.onChanged = { m.rendered() }
        peering = pr
        m.rendered()
        val early = synchronized(lock) { earlyPeering.filter { it.first == them }.also { earlyPeering.clear() } }
        early.forEach { (_, bytes) -> pr.frame(bytes) }
        // off the caller's thread: the first leg sends
        Thread { pr.begin() }.start()
    }

    /** The kernel calls a [Peering] makes, bracketed like every other. */
    private fun peeringKern(p: Participant): Peering.Kern = object : Peering.Kern {
        override fun backPointers(): List<ByteArray> =
            Gateway.call("backPointers", { emptyList() }) { p.backPointers() } ?: emptyList()

        override fun propose(
            other: String,
            presence: String,
            otherBack: List<ByteArray>,
        ): Peering.Result<ByteArray> = peered("proposePeering") {
            // no replication commitment: `wire-format.md` §4.4 field 6 is
            // optional and what it promises is an operator's to offer
            p.proposePeering(Gateway.unhex(other), Gateway.unhex(presence), otherBack, null)
        }

        override fun reviewAndSign(body: ByteArray): Peering.Result<ByteArray> =
            peered("reviewAndSignPeering") { p.reviewAndSignPeering(body) }

        override fun envelope(
            body: ByteArray,
            entries: List<Pair<String, ByteArray>>,
        ): Peering.Result<ByteArray> = peered("peeringEnvelope") {
            val mine = p.signBody(body)
            val all = entries.map { (who, e) -> uniffi.rhtn_ffi.SignedEntries(Gateway.unhex(who), e) } +
                uniffi.rhtn_ffi.SignedEntries(p.me(), mine)
            uniffi.rhtn_ffi.peeringEnvelope(body, all)
        }

        override fun take(envelope: ByteArray): Peering.Result<String> =
            peered("takePeering") { Gateway.hex(p.takePeering(envelope)) }
    }

    /**
     * **The bootstrap QR's payload** (D1a): this device's identifier and
     * the kind of transaction, and nothing of the ceremony's own anchor.
     *
     * **This one is not a §14.3 object and the Gateway.shell owns it.** §14.3 fixes
     * the ANCHORED exchange, which is D2's; the bootstrap carries only what
     * design §7.1.1 needs to start a conversation between two people who
     * have not yet exchanged a contribution, so there is nothing here for
     * the kernel to check and nothing for it to encode. The Gateway.shell's own
     * framing, versioned so a later one can differ.
     */
    fun bootstrap(by: String): ByteArray? {
        val m = meet ?: return null
        val me = Gateway.loaded[by]?.me() ?: return null
        val kind = when (m.adopt) {
            Meet.Adopt.NONE -> 0
            Meet.Adopt.THEM_UNDER_ME -> 1
            Meet.Adopt.ME_UNDER_THEM -> 2
        }
        // which subnet, and what this device calls itself and the subnet
        // there, as its claim: the scope chosen as patron, or the subnet
        // this device acts in otherwise, so a regular meeting carries a
        // name too (`light-client-requirements.md` §7)
        val scope = m.scope() ?: Gateway.subnets(by).firstOrNull { it.id.hex() == Gateway.subnet(by) }?.let { Meet.Scope.of(it) }
        // **and the name, so it is visible from the scan** [author,
        // 2026-10-29]: the one published there, else the default
        val alias = scope?.ownAlias?.takeIf { it.isNotBlank() } ?: Gateway.defaultAlias(by)
        return Bootstrap.encode(me, kind, m.kind.peering, scope, alias)
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
        val read = Bootstrap.decode(bytes)
            ?: return "that code is not a meeting invitation this build knows"
        val m = meet ?: return "no meeting is open here"
        // the code is who is being met: the first one names them
        m.counterparty(read.me)?.let { return it }
        m.theirAdopt(
            when (read.kind) {
                1 -> Meet.Adopt.THEM_UNDER_ME
                2 -> Meet.Adopt.ME_UNDER_THEM
                else -> Meet.Adopt.NONE
            },
        )
        // and whether it asked for a peering, which is the inviter's
        // choice and the brief's to state (A3; design §6.3)
        m.theirPeering(read.peering)
        // and what it claimed about where it leads, which the brief states
        // as a claim (`light-client-requirements.md` §7)
        m.invitedBy(Meet.Claim(read.subnet, read.alias, read.subnetAlias))
        // and what they call themselves, from the scan [author, 2026-10-29]
        m.heard(read.alias)
        return null
    }

    /**
     * What to show on the screen at D2: the contribution first, then the
     * meeting id once the counterparty's contribution is in
     * (`wire-format.md` §14.3.1's two steps, in order).
     *
     * **The kernel decides which**, because only it knows whether it has
     * read the other side's contribution yet.
     */
    fun optical(by: String): ByteArray? {
        val p = Gateway.loaded[by] ?: return null
        return Gateway.call(if (opticalTaken) "transcriptConfirm" else "opticalContribution", { null }) {
            if (opticalTaken) p.transcriptConfirm() else p.opticalContribution()
        }
    }

    /**
     * Bytes the selfie camera read. Null where they were taken, a reason
     * otherwise — and a reason here stops the ceremony, because every
     * refusal at this step is either a bearer disagreeing with a screen or
     * a party who is not the one in Gateway.front of you.
     */
    /**
     * **The optical exchange's trace to this side's Attachments.nominees** (kind 21)
     * [author, 2026-10-19], one message per event and never batched: the
     * flow drains the exchange as each read lands and hands the events
     * here, where each goes to the Attachments.nominees this side engaged.
     *
     * **A failure is not the meeting's end.** A nominee that cannot be
     * reached is a witness that does not appear (design §7.1); what the
     * trace buys is the arrival pattern on the clocks of the Attachments.nominees it
     * does reach, and the exchange itself is unaffected.
     */
    fun opticalTrace(by: String, events: List<OpticalExchange.Event>) {
        if (events.isEmpty()) return
        val p = Gateway.loaded[by] ?: return
        val m = meet ?: return
        // **off the thread that drives the exchange, always** [bench,
        // 2026-10-24]: each event is a relayed request to the nominee's
        // node, 100 ms at the median and up to seven seconds, and the
        // exchange's screen called this 210 times from its own thread — the
        // code stopped cycling and the preview froze for the duration (run
        // 1bd1c15-direct-1). One thread, so the events keep their order,
        // which is what the witness attests from
        traceWork.execute {
            for (e in events) {
                courier(by, p).trace(e)?.let { why ->
                    Diag.warn("optical.trace", "why" to why)
                    // said once a meeting, not once an event: a nodeless run
                    // said it 130 times (2026-10-24)
                    if (traceRefusedSaid.compareAndSet(false, true)) {
                        m.note("a nominee could not be sent the exchange's shape.")
                    }
                    return@execute
                }
            }
        }
    }

    fun takeOptical(by: String, bytes: ByteArray): String? {
        val p = Gateway.loaded[by] ?: return "the kernel is not running"
        val m = meet ?: return "no meeting is open here"
        if (!opticalTaken) {
            return Gateway.call("takeOptical", { it.reason }) {
                // the kernel pins the key the code carries and names the
                // party by its hash; the flow learns who that is, or
                // refuses a code from anyone but the one it already knows
                val who = p.takeOptical(bytes)
                m.counterparty(Gateway.hex(who))?.let { return@call it }
                adopt(by, who)
                opticalTaken = true
                m.note("their contribution is in; showing the meeting id.")
                null
            }
        }
        if (transcriptTaken) return null
        var refused: String? = null
        val id = Gateway.call("takeTranscript", { refused = it.reason; null }) { p.takeTranscript(bytes) }
            ?: return refused ?: "the transcript was refused"
        transcriptTaken = true
        m.note("the two meeting ids agree: ${Gateway.hex(id).take(16)}…")
        // THE BEARER'S TURN. The optical step is what the integrity
        // rests on (§14.3.1); from here a radio carries the bulk,
        // and everything it carries is checked against what the
        // screens showed.
        carryIntent(m, id)
        return null
    }

    /**
     * **The counterparty, adopted as this device's lasting knowledge** at
     * the point its key is pinned — which both sides do at the optical
     * step, alike [author, 2026-10-08]: showing the invitation against
     * scanning it is an incident of the world and not a fact the rest of
     * the ceremony carries. Before this the provision had to name whoever
     * a device could meet; now the meeting is what introduces them
     * (`wire-format.md` §14.3.1, design §12.3), and the provisioned Attachments.peer
     * remains only for messaging before any meeting.
     */
    private fun adopt(by: String, who: ByteArray) {
        val key = Gateway.hex(who)
        synchronized(lock) {
            Attachments.peer = who
            Attachments.peerKey = key
            if (Attachments.peerName.isEmpty() || Attachments.peerName == "the other person") Attachments.peerName = key.take(8)
        }
        Gateway.front.peerKnown(by, key, Attachments.peerName)
    }

    /** Who this device is talking to: the meeting's counterparty while one
     *  is open, the adopted or provisioned Attachments.peer otherwise. */
    private fun counterparty(): ByteArray? =
        meet?.counterpartyKey?.let { runCatching { Gateway.unhex(it) }.getOrNull() } ?: Attachments.peer

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
     * the bearer to the Gateway.shell, so this uses the invitation's own asymmetry:
     * the party that showed it offers, the party that read it seeks.
     */
    private fun carryIntent(m: Meet, ceremony: ByteArray) {
        val p = Gateway.loaded[m.by] ?: return
        val radio = BleBearer(Gateway.appContext ?: return)
        // ONE inbound handler for the whole conversation. Each phase is
        // apart in the carriage (`Carriage.Phase`), so a packet arriving
        // is just dropped into its phase and the step waiting on that phase
        // is attempted — a late capture-key packet cannot complete the intent
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
                    drainBearer(p, m)
                }
            } catch (e: java.util.concurrent.RejectedExecutionException) {
                // the meeting ended between the check and the submit
            }
        }
        val why = when (m.role) {
            Meet.Role.INITIATOR -> radio.offer(inbound, ceremony)
            Meet.Role.RESPONDER -> radio.seek(inbound, ceremony)
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
            sentCarriages.set(0)
        }
        Thread {
            // THE LINK FIRST. offer and seek return as the radio starts,
            // not as a Attachments.peer connects and subscribes, and a packet sent
            // before the subscription goes nowhere while reporting success
            // (BleBearer). So the intent waits, bounded, on the link coming
            // ready; a link that does not is a stopped meeting with a
            // reason, never an intent that silently did not cross.
            if (!awaitLink(radio, m, "intent")) return@Thread
            val mine = Gateway.call("intentCarriage", { e ->
                m.stop("the intent could not be built: ${e.reason}")
                null
            }) { p.intentCarriage() } ?: return@Thread
            if (carriage?.send(mine, Carriage.Phase.INTENT) == true) {
                m.note("intent sent: ${mine.size} message(s) over the radio.")
                // the capture starts once theirs is in as well
                m.intentSent()
                // **and this side's candidates behind it** (design §12.6.3):
                // the route, anchored, so the two leave with a path to each
                // other that no node carried — the one the conversation's
                // legs and an adoption after a nodeless record take. Not
                // fatal: with nothing to offer the relay stands, and it is said
                Gateway.call("candidateOffer", { e -> m.note("no candidates offered: ${e.reason}"); null }) {
                    p.candidateOffer()
                }?.let { offer ->
                    if (carriage?.send(listOf(offer), Carriage.Phase.CANDIDATES) == true) {
                        m.note("my candidates are sent.")
                    }
                }
            } else {
                m.stop("the radio would not take the intent; nothing crossed")
            }
        }.start()
    }

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
     * in the kernel until drained, and no node carries them. Drained after
     * every step this side takes, every carriage taken and every message
     * that arrived over the network, since each can produce one; one
     * message per phase, so the receiver can take each as it lands.
     *
     * **A carriage the radio will not take ends the ceremony**: the kernel
     * holds no copy, so the message is gone and the counterparty is
     * waiting on a phase that exists nowhere, which is not a failure to
     * leave to the person [author, 2026-10-07].
     */
    private fun sendCarriages(p: Participant) {
        val live = carriage ?: return
        val out = Gateway.call("carriages", { listOf() }) { p.carriages() }
        for (bytes in out) {
            val phase = Carriage.Phase.conversation(sentCarriages.getAndIncrement())
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
    /**
     * **Arrival drives the machine.** [drainBearerLocked] takes what
     * landed; the poll that acts on it runs here, outside `bearerLock`,
     * because A6 showed the alternative: MRK received the counterparty's
     * responses — `phase 7, 32 bytes, duplicates 0, refused 0` — and its
     * screen went on saying it was waiting for them, because the only
     * poll was the turn loop's and the loop was not turning
     * (2026-10-24). Setting a flag and waiting for a tick is one clock
     * too many; what arrived is itself the reason to look.
     *
     * **Outside the lock on purpose**: a poll can propose, hand over or
     * stop, each of which calls back into the kernel, and doing that
     * under `bearerLock` would put the bearer's own thread behind the
     * work it is trying to deliver. [Meet] guards its own state, so a
     * poll from here and one from the loop cannot interleave inside it.
     *
     * **This routes around the loop rather than repairing it** [author,
     * 2026-10-24, "A then B"]: the loop not turning is a separate defect
     * and `kernel.loop`/`kernel.turn` are in to find it.
     */
    private fun drainBearer(p: Participant, m: Meet) {
        drainBearerLocked(p, m)
        if (carriedPending.getAndSet(false)) m.poll(courier(m.by, p))
    }

    private fun drainBearerLocked(p: Participant, m: Meet): Unit = synchronized(bearerLock) {
        // SERIALIZED, because two threads reach here: the BLE inbound
        // callback when the counterparty's packets land, and the step
        // driver (runWindow) right after it sends. received()
        // is non-destructive, so without this both could read one completed
        // phase, both take it, and the second advance() would throw on a
        // callback thread (step has already moved). The lock makes the
        // second entry see the advanced step and do nothing.
        val live = carriage ?: return
        // **the prekey handover is taken the moment it lands, at any
        // step**: the kernel keeps it only while the ceremony is active,
        // and the ceremony ends at finalization on the kernel's own clock
        live.received(Carriage.Phase.PREKEY)?.let { set ->
            set.firstOrNull()?.let { bytes ->
                Gateway.call("takePrekeyCarriage", { e -> m.note("their prekey handover was refused: ${e.reason}") }) {
                    p.takePrekeyCarriage(bytes)
                    m.note("their prekeys are in: a channel can open later without a node.")
                }
            }
            live.discard(Carriage.Phase.PREKEY)
        }
        // **the window opens with the request** (design §7.1 item 4), the
        // first step after the codes, and **the conversation enters with
        // it** [author, 2026-10-24]: the queries go the moment the window is
        // open, while the capture runs beside them (items 5 and 6). Refused
        // with `Waiting` while the counterparty's prune has not answered,
        // and asked again on the next turn of this drain
        if (m.step() == Meet.Step.WINDOW && m.openWindow(courier(m.by, p)) &&
            conversationStarted.compareAndSet(false, true)
        ) {
            startConversation(p, m)
        }
        // **the counterparty's candidates, off the bearer** (design §12.6.3):
        // checked against the ceremony's anchor and dialled, so the two
        // hold a path to each other that no node carried — the one the
        // conversation's legs and an adoption after the record take when
        // there is no node [bench, 2026-10-24]
        live.received(Carriage.Phase.CANDIDATES)?.let { set ->
            set.firstOrNull()?.let { bytes ->
                Gateway.call("takeCandidateOffer", { e -> m.note("their candidates were refused: ${e.reason}") }) {
                    val held = p.takeCandidateOffer(bytes)
                    m.note(if (held) "state: a direct path to ${m.counterpartyName} is open." else "state: their candidates did not open a path; the relay stands.")
                }
            }
            live.discard(Carriage.Phase.CANDIDATES)
        }
        // the counterparty's capture key, whenever it lands inside the
        // window: held for the seal, which waits on it and on the hold
        if (m.step() == Meet.Step.WINDOW) {
            live.received(Carriage.Phase.CAPTURE_KEY)?.let { set ->
                set.firstOrNull()?.let { bytes ->
                    // the carried `CaptureKeyHandover` is checked against
                    // this ceremony's id in the kernel, and their key is
                    // what comes back. It seals MY capture of them
                    // (design §7.5.2.6)
                    val k = Gateway.call("takeCaptureKeyCarriage", { e -> m.stop("the capture could not be sealed: ${e.reason}"); null }) {
                        p.takeCaptureKeyCarriage(bytes)
                    }
                    if (k != null) {
                        theirCaptureKey = k
                        m.note("their capture key is in.")
                    }
                    // the bearer's assembly is zeroed: a key opens a
                    // likeness of a person (design §7.5.2)
                    set.forEach { it.fill(0) }
                    live.discard(Carriage.Phase.CAPTURE_KEY)
                    trySeal(p, m)
                }
            }
        }
        when (m.step()) {
            Meet.Step.OPTICAL -> live.received(Carriage.Phase.INTENT)?.let { set ->
                val to = counterparty() ?: return m.stop("the codes did not name who this is with")
                // every refusal here is the kernel catching a bearer
                // that disagrees with the screens (§14.3.2), which is
                // the one thing the anchor is for
                Gateway.call("takeIntentCarriage", { e -> m.stop("the carried intent was refused: ${e.reason}") }) {
                    val taken = p.takeIntentCarriage(to, set)
                    m.note("their intent is in, with $taken continuation(s).")
                    // the capture starts once ours has gone as well
                    m.intentReceived()
                }
            }
            // the conversation: every phase from CONVERSATION up that has
            // arrived whole is one message of the counterparty's, taken
            // and its phase discarded so the sender's count can wrap.
            // What a taken step owes the counterparty goes straight back
            Meet.Step.WINDOW, Meet.Step.REVIEW -> {
                var taken = 0
                for (phase in Carriage.Phase.CONVERSATION until Bearer.PHASES) {
                    val set = live.received(phase) ?: continue
                    for (bytes in set) {
                        Gateway.call("takeCarriage", { e -> m.note("a carriage was refused: ${e.reason}") }) {
                            p.takeCarriage(bytes)
                        }
                        taken += 1
                    }
                    live.discard(phase)
                }
                if (taken > 0) {
                    // **what arrived has to wake the poll.** These are the
                    // conversation's own messages and nodeless they come
                    // only this way, with no kernel event behind them, so
                    // without this the flow never looks again
                    m.carried()
                    sendCarriages(p)
                    // and the poll itself runs in [drainBearer], once this
                    // lock is given up
                    carriedPending.set(true)
                }
            }
            else -> {}
        }
    }

    /**
     * Seal this device's capture under the counterparty's key, once both
     * are in: the hold's window elapsed ([captureWindowDone]) and the key
     * landed ([theirCaptureKey]). Called from whichever arrives second, and
     * a no-op otherwise. THE KEY IS LET GO HERE, sealed or not: it opens a
     * likeness of a person, and the kernel wipes its own copies as the call
     * returns (design §7.5.2).
     */
    private fun trySeal(p: Participant, m: Meet) {
        val sealed = synchronized(sealLock) {
            if (!captureWindowDone.get()) return
            val k = theirCaptureKey ?: return
            theirCaptureKey = null
            try {
                Gateway.call("captureSeal", { e -> m.stop("the capture could not be sealed: ${e.reason}"); false }) {
                    p.captureSeal(k)
                    m.note("captures sealed under their key.")
                    m.captureSealed()
                    true
                }
            } finally {
                k.fill(0)
            }
        }
        // the seal is what the proposal and the handover were waiting on;
        // outside the lock, since a poll calls back into the kernel
        if (sealed) {
            // **the camera goes back at the seal** [author, 2026-11-11],
            // not when the person taps Done. Nothing takes a frame after
            // this: the hold's prompts and the continuity probe both run
            // inside the window, and the window is what the seal closes.
            // Held past it, the camera is lit through the proposal, the
            // review, the signing and however long a Done screen stands —
            // and on hardware that permits one camera session at a time it
            // is what gave the bench **one ceremony per app start** (run
            // `18521b9-new-universe-1`: face-camera opens against closes
            // 2/0, 1/0, 2/1, 1/0 across the four phones, a Done screen
            // left standing on each).
            //
            // **`stopMeet` still releases it**, which is the backstop for
            // a ceremony that never reaches a seal — an abort, a stall, a
            // dismissal — and a second release costs nothing
            // ([FaceCamera.close] is safe to call twice).
            //
            // **A frame asked for after this reopens the camera**, which
            // [AndroidShell.capture] already does and retries: releasing
            // early is therefore recoverable, where holding on is not.
            Gateway.shell?.endCapture()
            m.poll(courier(m.by, p))
        }
    }

    /** The window's per-meeting state, cleared as a meeting starts or ends. */
    private fun resetWindow() {
        theirCaptureKey?.fill(0)
        theirCaptureKey = null
        captureWindowDone.set(false)
        conversationStarted.set(false)
        traceRefusedSaid.set(false)
        synchronized(lock) {
            earlyAdoption.clear()
            earlyPeering.clear()
        }
    }

    /**
     * **D4 inside the window**: hand the counterparty the key that seals
     * its captures of me, then turn this device's hold for the window
     * (design §7.1 item 5; `Participant.captureBegin`/`captureTick`), one
     * frame a turn on the selfie camera, while the drain opens the window
     * and runs the conversation beside it. The keys cross as
     * `CaptureKeyHandover` (`wire-format.md` §14.3.2), one each way as the
     * one message of the CAPTURE_KEY phase, anchored to the ceremony-id and
     * checked against it in the kernel when taken. The seal is taken by
     * [trySeal] once both the hold is done and their key is in, whichever
     * lands last.
     */
    fun runWindow(m: Meet) {
        val p = Gateway.loaded[m.by] ?: return
        // the handover carries my key in clear: once the bearer has
        // had it, this Gateway.shell keeps no copy, the packets it was cut
        // into included (design §7.5.2)
        val radio = ble ?: return m.stop("no radio carries the capture key")
        if (!awaitLink(radio, m, "capture key")) return
        val mine = Gateway.call("captureKeyCarriage", { e -> m.stop("capture: ${e.reason}"); null }) {
            p.captureKeyCarriage()
        } ?: return
        val sent = try {
            carriage?.send(listOf(mine), Carriage.Phase.CAPTURE_KEY, wipe = true) == true
        } finally {
            mine.fill(0)
        }
        if (!sent) return m.stop("the radio would not take the capture key; nothing crossed")
        m.note("my capture key is sent; capturing the counterparty across the window.")
        // **and the prekeys behind it** (`wire-format.md` §14.3.4): this
        // device's signed bundle and one one-time key, so the two can open
        // a channel afterwards with no node to ask. Not the record's
        // concern: a handover that will not cross is noted, and the meeting
        // goes on
        val prekeys = Gateway.call("prekeyCarriage", { e -> m.note("no prekey handover: ${e.reason}"); null }) {
            p.prekeyCarriage()
        }
        if (prekeys != null && carriage?.send(listOf(prekeys), Carriage.Phase.PREKEY) != true) {
            m.note("the radio would not take the prekey handover; a later message will need a node.")
        }
        drainBearer(p, m)
        // THE HOLD, turned from here for the window: each tick is one short
        // call into the kernel — a frame and the detector — so the
        // conversation's own calls interleave with it rather than waiting
        // behind one call that held the kernel's thread for the window
        val lost = { e: Refused.Reason -> m.stop("the capture was lost: ${e.reason}") }
        refusal("captureBegin") { p.captureBegin() }?.let { return m.stop("capture: $it") }
        var said = false
        var face: Boolean? = null
        var faceSaidAt = 0L
        while (m.step() == Meet.Step.WINDOW || m.step() == Meet.Step.REVIEW) {
            val turn = Gateway.call("captureTick", { lost(it); null }) { p.captureTick() } ?: return
            if (turn.templateReady && !said) {
                said = true
                m.note("state: a frame yielded a template; the queries can go.")
            }
            // the face, as state: said when it comes or goes, and every
            // five seconds while it is absent (developer wording until the
            // UX pass [author, 2026-10-24])
            val now = System.currentTimeMillis()
            if (face != turn.face || (!turn.face && now - faceSaidAt > 5_000L)) {
                face = turn.face
                faceSaidAt = now
                m.note(if (turn.face) "state: a face is in frame." else "state: no face in frame; absent ${turn.awayMs / 1000u} s of the window so far.")
            }
            if (turn.done) {
                m.note("state: the capture's window has elapsed, ${turn.frames} frame(s) kept.")
                captureWindowDone.set(true)
                trySeal(p, m)
                return
            }
            Thread.sleep(turn.nextMs.toLong().coerceIn(50L, 2_000L))
        }
    }

    private fun courier(by: String, p: Participant): Meet.Courier = object : Meet.Courier {
        // each step's leg to the counterparty is a carriage for the
        // bearer, drained right after the step whether it went or was
        // refused: a refused propose that is `Waiting` has sent nothing,
        // and a step that went has left its carriage in the kernel
        override fun engage(): String? = refusal("converseEngage") { p.converseEngage() }
        override fun trace(e: OpticalExchange.Event): String? =
            refusal("opticalTrace") {
                p.opticalTrace(
                    when (e) {
                        is OpticalExchange.Event.Shape ->
                            TraceStep.Shape(e.mine.toULong(), e.theirs.toULong())
                        is OpticalExchange.Event.Inbound -> TraceStep.Inbound(e.index.toULong())
                        is OpticalExchange.Event.Outbound -> TraceStep.Outbound(e.index.toULong())
                    },
                )
            }
        override fun open(): String? = refusal("converseOpen") { p.converseOpen() }.also { sendCarriages(p) }
        /** This device's name to the counterparty, on the channel the
         *  handover left [author, 2026-10-29]: the responder's only way
         *  to be named before a profile exists, and the inviter's again. */
        override fun hello(): String? {
            val them = meet?.counterpartyKey ?: return "no counterparty yet"
            val name = Gateway.defaultAlias(by) ?: return null
            return Gateway.call("send", { it.reason }) {
                p.send(Gateway.unhex(them), Adoption.KIND, Adoption.Frame.hello(name))
                null
            }
        }
        override fun queries(): String? = refusal("converseQueries") { p.converseQueries() }.also { sendCarriages(p) }
        override fun gathered(): String? = refusal("converseGathered") { p.converseGathered() }.also { sendCarriages(p) }
        override fun propose(): String? = refusal("conversePropose") { p.conversePropose() }.also { sendCarriages(p) }
        override fun progress(): Meet.Progress? =
            Gateway.call("progress", { null }) { p.progress() }?.let { pr ->
                Meet.Progress(
                    proposer = pr.proposer,
                    queriesOutstanding = pr.queriesOutstanding.toInt(),
                    witnessesOutstanding = pr.witnessesOutstanding.toInt(),
                    opened = pr.opened,
                    handed = pr.handed,
                    proposable = pr.proposable,
                    noWitness = pr.noWitness,
                    waitingOn = pr.waitingOn,
                    attesting = pr.attesting.map { Gateway.hex(it) },
                    declined = pr.declined.map { Gateway.hex(it) },
                    backFrom = pr.backFrom.map { Gateway.hex(it) },
                    theirResponses = pr.theirResponses,
                    proposed = pr.proposed,
                    signed = pr.signed.map { Gateway.hex(it) },
                    refused = pr.refused.map { Gateway.hex(it) },
                )
            }
    }

    /**
     * Enter the conversation as the window opens: the Attachments.nominees and the
     * verifier rows for the screen first, then the kernel's open and its
     * queries, off the thread the bearer's packet arrived on, since each
     * sends over the network. The flow opens once; a second entry is its
     * own no-op.
     */
    private fun startConversation(p: Participant, m: Meet) {
        Thread {
            Gateway.call("Attachments.nominees", { null }) { p.nominees() }?.let { n ->
                m.nominated(n.mine.map { Gateway.hex(it) }, n.theirs.map { Gateway.hex(it) })
            }
            val picked = Gateway.call("selectVerifiers", { e -> m.note("selection refused: ${e.reason}"); null }) {
                p.selectVerifiers()
            }
            if (picked != null) {
                m.selected(picked.map { Meet.Chosen(Gateway.hex(it.verifier), basisOf(it.basis)) })
                if (picked.isEmpty()) {
                    m.note("no verifier is required: the counterparty handed over")
                    m.note("no records, so wire-format §5.2 obliges none.")
                }
            }
            m.converse(courier(m.by, p))
        }.start()
    }

    /** `wire-format.md` §5.5's `selection_basis`, as the Gateway.shell's own words
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
    private fun turnOf(m: Meet, c: Conversation): Meet.Turn = when (c) {
        is Conversation.Finalized -> {
            // nothing more is met over this bearer: stop offering, so a
            // later scanner in the room does not find this phone first
            ble?.quiet()
            // who the record says witnessed it, from the signed body
            val named: List<List<ByteArray>> = Gateway.loaded[m.by]?.recordWitnesses(c.txid) ?: listOf()
            val witnesses: List<String> = named.mapNotNull { pair: List<ByteArray> ->
                pair.firstOrNull()?.let { key: ByteArray -> Gateway.hex(key) }
            }
            Meet.Turn.Finalized(Gateway.hex(c.txid), witnesses)
        }
        is Conversation.Reviewed ->
            c.refused?.let { Meet.Turn.BodyRefused(it.code.toInt(), it.why) } ?: Meet.Turn.Other
        is Conversation.Signed ->
            c.refused?.let { Meet.Turn.SignerRefused(Gateway.hex(c.signer), it.code.toInt(), it.why) } ?: Meet.Turn.Other
        else -> Meet.Turn.Other
    }

    /** What a conversation event refused, for the diagnostics: a signing
     *  refusal's code and words, or a stranger's message's reason. */
    internal fun refusalIn(c: Conversation): String? = when (c) {
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
                val v = Gateway.hex(notice.verifier).take(16)
                meet?.querySurfaced(v) ?: Gateway.front.note("· a query about you was answered by $v…")
            }
            is Told.NomineesOutnumbered ->
                meet?.note("· witnesses: ${notice.mine} of mine, ${notice.theirs} of theirs")
            else -> Gateway.front.note("· $notice")
        }
    }

    /** End the ceremony in progress. */
    fun stopMeet(reason: String) {
        meet?.stop(reason)
        val work = synchronized(lock) {
            meet = null
            resetWindow()
            // **the adoption goes with the meeting it rested on**: a frame
            // of the next meeting's would otherwise reach the last one's,
            // which refuses it for coming from the wrong party
            adoption = null
            peering = null
            opticalTaken = false
            transcriptTaken = false
            ble?.close()
            ble = null
            carriage = null
            bearerWork.also { bearerWork = null }
        }
        // the bearer's thread goes with the meeting; a send still in the
        // gate is not waited for, since the link it was for is closed
        work?.shutdownNow()
        // the camera goes back with the meeting: held behind a dead
        // ceremony it is the kind of thing nobody consented to
        Gateway.shell?.endCapture()
    }

    /** The meeting in progress, where one is open. */
    fun meet(): Meet? = meet

    /** The adoption after the record, where the meeting carried one. */
    fun adoption(): Adoption? = adoption

    /** The peering the meeting carried, once one has begun (A3). */
    fun peering(): Peering? = peering

    /** Whether the counterparty's contribution is in, which is what the
     *  screen's second QR at D2 is: the meeting id rather than ours. */
    fun opticalTaken(): Boolean = opticalTaken

    /** **That the loop is about to ask for an event**, on the clock. With
     *  [turnHeartbeat] this brackets the one call in the loop that can
     *  block: an entry with no heartbeat after it is a park in
     *  `nextEvent`, and no entry at all is a loop that is not running. */
    internal fun turnEntry(d: TurnDiag, turns: Long) {
        if (!Diag.enabled()) return
        val now = System.currentTimeMillis()
        if (now - d.lastEntryAt < turnDiagMs) return
        d.lastEntryAt = now
        Diag.event("kernel.turn", "turns" to turns)
        // the heap beside it, on the same clock: a number that climbs across
        // a run is a leak, and one that does not rules it out [bench,
        // 2026-10-24]
        val rt = Runtime.getRuntime()
        Diag.event(
            "Gateway.shell.memory",
            "java_kb" to (rt.totalMemory() - rt.freeMemory()) / 1024,
            "native_kb" to android.os.Debug.getNativeHeapAllocatedSize() / 1024,
        )
    }

    /** [Meet.turnState] into the diagnostics, on change or on the clock;
     *  `meet=null` where there is no meeting, so that a turn always says
     *  something and silence can only mean the loop. */
    internal fun turnHeartbeat(d: TurnDiag, m: Meet?) {
        if (!Diag.enabled()) return
        val now = System.currentTimeMillis()
        val s = m?.turnState(now) ?: "meet=null"
        if (s == d.lastTurnState && now - d.lastTurnAt < turnDiagMs) return
        d.lastTurnState = s
        d.lastTurnAt = now
        Diag.event("meet.turn", "state" to s)
    }


    /**
     * **What of an identity's events is a meeting's** [restructure step 4,
     * 2026-11-14]: a frame of the meeting held as `id`, or of the adoption
     * or peering it carried, taken here; true. A meeting another identity
     * holds is not acted on, and said so. Everything else is the loop's.
     */
    fun took(id: String, p: Participant, e: Event): Boolean {
        val m = meet?.takeIf { it.by == id }
        val another = meet != null && m == null
        when (e) {
            is Event.Payload -> {
                val from = Gateway.hex(e.from)
                val said = if (e.kind == Adoption.KIND) Adoption.Frame.heard(e.bytes) else null
                if (said != null) {
                    if (m != null && m.counterpartyKey == from) m.heard(said)
                    return true
                }
                if (e.kind != Adoption.KIND && e.kind != Peering.KIND) return false
                if (another) {
                    Gateway.front.note("· a message of a meeting arrived for an identity not in focus, and was not acted on")
                    return true
                }
                if (e.kind == Adoption.KIND) {
                    val a = adoption
                    when {
                        a == null -> synchronized(lock) { earlyAdoption += from to e.bytes }
                        from != a.counterparty -> Gateway.front.note("· a message of an adoption arrived from somebody who is not party to this one")
                        else -> a.frame(e.bytes)
                    }
                } else {
                    val pr = peering
                    when {
                        pr == null -> synchronized(lock) { earlyPeering += from to e.bytes }
                        from != pr.counterparty -> Gateway.front.note("· a message of a peering arrived from somebody who is not party to this one")
                        else -> pr.frame(e.bytes)
                    }
                }
                return true
            }
            is Event.Answered -> {
                val v = e.answer ?: return false
                m?.responded(Gateway.hex(e.from), verdictOf(v))
                return true
            }
            is Event.Conversed -> {
                if (m == null) return false
                m.conversed(e.what, turnOf(m, e.step))
                // a step taken on a message from a witness can owe the
                // counterparty one: the last signature in, the record out
                sendCarriages(p)
                return true
            }
            else -> return false
        }
    }

    /**
     * THE FLOW'S CLOCK, once a turn of `id`'s loop. The conversation is
     * polled every tick while it is open, after whatever arrived: this is
     * where the proposer learns its queries are answered and proposes,
     * where the responder hands over, and where the body's arrival moves
     * to review. The heartbeat is said every turn whatever the flow is
     * doing, so its silence means one thing: the loop is not turning.
     */
    internal fun tick(id: String, p: Participant, diag: TurnDiag) {
        val tick = meet?.takeIf { it.by == id }
        turnHeartbeat(diag, tick)
        tick?.let { m ->
            // a conversation phase that landed before the flow reached
            // the conversation is taken on the tick, as is one whose
            // packet arrived while the step driver held the lock
            val t = m.step()
            if (t == Meet.Step.WINDOW || t == Meet.Step.REVIEW) {
                drainBearer(p, m)
            }
            // **only where it has something to do**: an arrival since
            // the last look, or the patience run out. Polling every
            // turn had the proposer asking the kernel 28 times in 3 s
            // on the bench [client review, 2026-10-11].
            if (m.pollDue()) m.poll(courier(id, p))
            // **the record is what an adoption rests on**, so D7
            // begins where the ceremony ends; begun once, and nothing
            // where the meeting carried none
            if (t == Meet.Step.DONE) {
                adoptionBegins(id)
                // and the peering the invitation's checkbox asked for,
                // which is independent of the adoption
                peeringBegins(id)
                // **and the departure a concluded rotation owes**
                // [author, 2026-11-07]: the old key leaves the subnet
                // that rotated once the new one is adopted into its
                // place, so this is the moment it is owed
                Identities.concludeRotation()
            }
            if (m.stepStale()) {
                m.stop("nothing has moved for ${Meet.STALE_MS / 1000} s at ${m.step().name.lowercase()}; the other phone may have stopped")
            }
        }
    }
}
