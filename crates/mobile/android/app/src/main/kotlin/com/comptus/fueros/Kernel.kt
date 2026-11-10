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

    /**
     * **Every identity this device holds, open at once**, by its id
     * ([Identities]). A key rotation replaces the key one subnet knows and
     * leaves the others live (design §9.0), so a device that has rotated
     * holds a key still usable elsewhere [author, 2026-11-07].
     *
     * Each has storage of its own ([AndroidShell]'s scope): two kernels
     * over one directory would each overwrite the other's archive.
     */
    private val loaded = java.util.concurrent.ConcurrentHashMap<String, Participant>()

    /**
     * **Which identity acts** — a meeting's and an operator action's
     * ([Identities.needsTheActive]), chosen by press and hold on the
     * nameplate. Null only where the device holds none.
     */
    @Volatile private var activeId: String? = null

    /**
     * **The identity that acts.** Every use of this in the shell means
     * the active one, which is what a meeting and an operator action
     * take; viewing and acting inside a subnet are every identity's at
     * once and read [loaded] instead.
     */
    private val participant: Participant?
        get() = activeId?.let { loaded[it] }
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

    /** The adoption after the record (D7), or null where the meeting
     *  carried none. Outlives the ceremony: the record closes the
     *  ceremony and the adoption rests on it. */
    @Volatile private var adoption: Adoption? = null
    @Volatile private var peering: Peering? = null

    /** Whether the counterparty's contribution has been read off their
     *  screen, which is what decides the second QR from the first. */
    @Volatile private var opticalTaken = false
    /** Whether the transcript has been taken for the meeting open: a
     *  second is the screen repeating itself, and starts no second bearer. */
    @Volatile private var transcriptTaken = false

    /** The radio and the carriage over it, once the meeting opens one. */
    @Volatile private var ble: BleBearer? = null
    @Volatile private var carriage: Carriage? = null
    /** How many conversation carriages this side has sent over the bearer
     *  in this ceremony: what picks each one's phase ([Carriage.Phase.conversation]). */
    private val sentCarriages = java.util.concurrent.atomic.AtomicInteger(0)
    /** The counterparty's capture key, from the bearer until the seal takes
     *  it; wiped the moment it is used or the meeting ends. */
    @Volatile private var theirCaptureKey: ByteArray? = null
    /** Whether this device's hold has run its window: the seal waits on
     *  this and on [theirCaptureKey], whichever lands last. */
    private val captureWindowDone = java.util.concurrent.atomic.AtomicBoolean(false)
    /** The conversation is entered once per meeting, as the window opens. */
    private val conversationStarted = java.util.concurrent.atomic.AtomicBoolean(false)
    private val sealLock = Any()
    /** Frames of an adoption or a peering that arrived before this side began its own (see the loop). */
    private val earlyAdoption = mutableListOf<Pair<String, ByteArray>>()
    private val earlyPeering = mutableListOf<Pair<String, ByteArray>>()
    /** Whether this meeting has already said the trace could not go. */
    private val traceRefusedSaid = java.util.concurrent.atomic.AtomicBoolean(false)

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

    /** The application's context, kept for the radio the bearer needs. */
    @Volatile private var appContext: Context? = null

    /** Bind a screen: it is rendered against the state so far, then on
     *  every change. */
    fun bind(u: Front.Ui) = front.bind(u)

    /** Unbind a screen: it is rendered no more. */
    fun unbind(u: Front.Ui) = front.unbind(u)

    /** The meeting in progress, where one is open. */
    fun meet(): Meet? = meet

    /** The adoption after the record, where the meeting carried one. */
    fun adoption(): Adoption? = adoption

    /** The peering the meeting carried, once one has begun (A3). */
    fun peering(): Peering? = peering

    /** Whether the counterparty's contribution is in, which is what the
     *  screen's second QR at D2 is: the meeting id rather than ours. */
    fun opticalTaken(): Boolean = opticalTaken

    /**
     * **Enrol an instance of this operator's** (`Provision`,
     * `infra-client-requirements.md` §8.2, §8.3): fetch the transport key
     * it minted, check the proof that the key is *its*, then sign and hand
     * over the two records §4.4 says it cannot sign for itself, and last
     * the run that ends its wait.
     *
     * **The order is the records first and the run last**, because taking
     * the run is what makes it start serving: a node that began to serve
     * before its endpoint record arrived would be reachable and unfindable
     * at once.
     *
     * Each step is narrated as its own line, which is what the page shows.
     * **A step that fails stops the rest** — a half-enrolled instance is
     * one nobody asked for, and the lines say how far it got.
     *
     * @param at `host:port` of the instance's surface
     * @param tokenHex the enrolment token its configuration carries
     * @param credentials how many 48-hour delegations the run carries
     * @param endpoint the address to name in an endpoint record, or null
     * @param asn the provider's autonomous system, where the caller knows it
     * @param zone that provider's own name for the place, beside the ASN
     */
    fun enrol(
        at: String,
        tokenHex: String,
        credentials: Int,
        endpoint: String?,
        /** The subnet the instance serves, hex, or null for the one under
         *  this device's own key: the records are signed on that line's
         *  series (`wire-format.md` §7.6). */
        subnet: String? = null,
        /** **Where the machine is, as the launch chose it** [author,
         *  2026-11-06]: the ASN and the provider's own zone label, which
         *  ride in both records below so a horizon can be ordered by the
         *  pair later (design §3.4). Null from the hand-typed enrolment,
         *  which was told neither. */
        asn: Long? = null,
        zone: String? = null,
    ): List<String> {
        val p = participant ?: return listOf("the kernel is not running")
        val said = mutableListOf<String>()
        val token = runCatching { Provision.unhex(tokenHex.trim()) }.getOrElse {
            return listOf("the token is 64 lower-case hex digits")
        }
        if (token.size != 32) return listOf("the token is 64 lower-case hex digits")

        val got = Provision.fetch(at).getOrElse { return listOf("${it.message}") }
        val checks = call("enrolmentProofChecks", { false }) {
            uniffi.rhtn_ffi.enrolmentProofChecks(token, got.nonce, got.transportKey, got.proof)
        }
        if (!checks) {
            // **the one refusal that matters here**: something answered
            // that does not hold this configuration's token, so the key it
            // offered is not this instance's.  Nothing was signed
            return listOf(
                "the proof did not check: whatever answered at $at does not hold the token " +
                    "this instance was configured with, so the key it offered is not its own. " +
                    "Nothing was signed.",
            )
        }
        said += "transport key ${Provision.hex(got.transportKey).take(16)}…, proof checks"
        said += "it is ${got.phase}, holding ${got.credentials} credential(s)"

        // §4.4's two records, each signed here because the instance holds
        // no seed to sign its own
        if (endpoint != null) {
            val point = point(endpoint, asn, zone) ?: return said + "`$endpoint` is not host:port"
            val line = subnet?.takeIf { it.length == 64 }?.let { unhex(it) }
            val record = call("endpointRecord", { null }) { p.endpointRecord(line, listOf(point), 1u) }
                ?: return said + "the endpoint record was not signed"
            said += "endpoint record naming $endpoint: " +
                Provision.put(at, "/node/endpoint-record", record).fold({ it }, { "${it.message}" })
        }
        // **the anchor entry is always signed at enrolment** [author,
        // 2026-10-25]: its size is the kernel's to state (nothing beneath a
        // fresh instance), not the operator's to type
        run {
            val point = point(endpoint ?: at, asn, zone) ?: return said + "`$at` is not host:port"
            val line2 = subnet?.takeIf { it.length == 64 }?.let { unhex(it) }
            val entry = call("anchorEntryFresh", { null }) {
                p.anchorEntryFresh(line2, listOf(point), 1u)
            } ?: return said + "the anchor entry was not signed"
            said += "anchor entry, nothing beneath it yet: " +
                Provision.put(at, "/node/anchor-entry", entry).fold({ it }, { "${it.message}" })
        }

        val run = call("delegate", { null }) {
            p.delegate(got.transportKey, now(), credentials.toUInt())
        } ?: return said + "the run was not signed"
        run.forEachIndexed { i, credential ->
            said += "credential ${i + 1} of ${run.size}: " +
                Provision.put(at, "/node/run", credential).fold({ it }, { "${it.message}" })
        }
        said += "its administration page is at http://$at/"
        return said
    }

    /**
     * **Renew an instance's run before it lapses**
     * (`infra-client-requirements.md` §7): a run is 45 credentials of 48
     * hours, contiguous, and **the next starts where the last ends** —
     * which is what the instance reports, so nothing here guesses. The
     * proof is checked again first: a key that is not this instance's own
     * gets no signature of this operator's, renewal or not.
     *
     * Nothing else is re-signed. The endpoint record and the anchor entry
     * are what §4.4 asks of an operator once, and a renewal is about the
     * credential alone.
     */
    fun renew(
        at: String,
        tokenHex: String,
        credentials: Int,
        /** The subnet this instance's records were signed on, hex, or null
         *  for the line under this device's own key: a re-signed anchor
         *  entry must carry the same series as the standing one or it
         *  supersedes nothing (`wire-format.md` §2.3). */
        subnet: String? = null,
    ): List<String> {
        val p = participant ?: return listOf("the kernel is not running")
        val token = runCatching { Provision.unhex(tokenHex.trim()) }.getOrElse {
            return listOf("the token is 64 lower-case hex digits")
        }
        if (token.size != 32) return listOf("the token is 64 lower-case hex digits")
        val got = Provision.fetch(at).getOrElse { return listOf("${it.message}") }
        val checks = call("enrolmentProofChecks", { false }) {
            uniffi.rhtn_ffi.enrolmentProofChecks(token, got.nonce, got.transportKey, got.proof)
        }
        if (!checks) {
            return listOf(
                "the proof did not check: whatever answered at $at does not hold the token " +
                    "this instance was configured with. Nothing was signed.",
            )
        }
        if (got.runEnds == 0L) {
            return listOf(
                "that instance holds no run to renew: enrol it, which signs its first.",
            )
        }
        val said = mutableListOf(
            "it holds ${got.credentials} credential(s), the last ending at ${got.runEnds}",
        )
        // **contiguous**: the next window opens exactly where the run ends
        val run = call("delegate", { null }) {
            p.delegate(got.transportKey, got.runEnds.toULong(), credentials.toUInt())
        } ?: return said + "the renewal was not signed"
        run.forEachIndexed { i, credential ->
            said += "credential ${i + 1} of ${run.size}: " +
                Provision.put(at, "/node/run", credential).fold({ it }, { "${it.message}" })
        }
        // **the anchor entry, re-signed where the subtree moved under it**
        // (`infra-client-requirements.md` §4.4; [author, 2026-10-26]): the
        // size is the instance's own count, so the renewal — the visit an
        // operator already makes — compares it against what the entry
        // claims and re-signs at the next counter over the same points
        val claims = got.anchorClaims
        val seqno = got.anchorSeqno
        if (claims == null || seqno == null) {
            said += "no anchor entry is signed there: enrolling signs the first"
        } else if (claims == got.subtree) {
            said += "anchor entry: it claims a subtree of $claims and counts ${got.subtree}, so it stands"
        } else {
            val points = got.anchorPoints.map { Provision.unhex(it) }
                .ifEmpty { listOfNotNull(point(at)) }
            val line = subnet?.takeIf { it.length == 64 }?.let { unhex(it) }
            val entry = call("anchorEntry", { null }) {
                p.anchorEntry(line, points, got.subtree.toULong(), (seqno + 1).toUInt())
            }
            said += if (entry == null) {
                "the re-signed anchor entry was not signed"
            } else {
                "anchor entry re-signed at seqno ${seqno + 1}, a subtree of ${got.subtree} where it claimed $claims: " +
                    Provision.put(at, "/node/anchor-entry", entry).fold({ it }, { "${it.message}" })
            }
        }
        return said
    }

    // ---- intake: a party who was never met ----------------------------




    /**
     * **Take whatever an intake code carries.** An `AnchoredIntroduction`
     * (`wire-format.md` §2.4) is somebody's key and locator together with
     * the anchor that locator names and where it answers — which is what a
     * code handed over at a meeting carries [author, 2026-10-13] — and a
     * bare `Introduction` (§2.3) or a node's `EndpointRecord` (§7.6) are
     * taken too. The kernel decides which by parsing, so a person scans a
     * code without being asked what kind it is.
     *
     * One line saying what was taken, or why not.
     */
    fun takeIntake(bytes: ByteArray): String {
        val p = participant ?: return "the kernel is not running"
        var why = ""
        val took = call<uniffi.rhtn_ffi.Intake?>("takeIntake", { why = it.reason; null }) {
            p.takeIntake(bytes)
        }
        return if (took == null) {
            "that code is not one of these: $why"
        } else {
            "took ${took.kind}: ${Provision.hex(took.who).take(8)}"
        }
    }

    /**
     * **This client's own code to hand over** (`wire-format.md` §2.4), or
     * null where it has none to make: a card names the anchor of its own
     * locator and where that anchor answers, and a client that cannot say
     * either has nothing another party could act on.
     */
    fun ownCard(): ByteArray? {
        val p = participant ?: return null
        return call<ByteArray?>("ownCard", { null }) { p.ownCard() }
    }

    /**
      * A `NetworkPoint` for `host:port`, encoded, or null.
      *
      * **With the provider and the zone where the caller knows them**
      * [author, 2026-11-06]: a launch does, and what the next operator can
      * order by is what the records carry
      * (`light-client-requirements.md` §6). Null for both where this is a
      * hand-typed enrolment — a machine somebody already had is in a place
      * this screen was not told.
      */
    private fun point(hostPort: String, asn: Long? = null, zone: String? = null): ByteArray? {
        val host = hostPort.substringBeforeLast(':', "")
        val port = hostPort.substringAfterLast(':', "").toUShortOrNull() ?: return null
        if (host.isEmpty()) return null
        return call("networkPoint", { null }) {
            uniffi.rhtn_ffi.networkPoint(host, port, asn?.toULong(), zone?.takeIf { it.isNotBlank() })
        }
    }

    /** Seconds since the epoch, which is what a delegation's window is in. */
    private fun now(): ULong = (System.currentTimeMillis() / 1000).toULong()

    /**
     * Begin a ceremony, meeting or adopting as chosen, with whoever the
     * codes turn out to name (`wire-format.md` §14.3). A ceremony already
     * live is returned as-is.
     */
    fun startMeet(kind: Meet.Kind, role: Meet.Role): Meet? {
        val p = participant ?: return null
        val retention = call("retention_years", { 2UL }) { p.retentionYears() }
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
            // (`wire-format.md` §14.3.1), the provision's peer being the
            // payload demo's and no part of a ceremony
            resetWindow()
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
            // **the nominees are engaged with the intent, before the code
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
     * **Engage this side's nominees** (kind 20), retried while the
     * bindings land: the first attempt on a nominee whose session
     * material is not held asks for it, so a transient absence is not an
     * unavailable witness (`light-client-requirements.md` §1.1). The
     * attempts stop once every nominee is reached, the window opens, or
     * the tries run out — the kernel closes the set when the ceremony-id
     * fixes, and reports what it reached.
     */
    private fun engageNominees(p: Participant, m: Meet) {
        val c = courier(p)
        for (attempt in 0 until Engagement.TRIES) {
            if (m.step() == Meet.Step.STOPPED) return
            c.engage()?.let { why ->
                Diag.warn("witness.engage", "why" to why)
                return
            }
            val reached = call("nomineesEngaged", { null }) { p.nomineesEngaged() } ?: return
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

    // ---- D7, the adoption the meeting carried --------------------------

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
    fun adoptionBegins() {
        val p = participant ?: return
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
                call("send", { e -> Diag.warn("adopt.send", "refused" to e.reason); false }) {
                    p.send(unhex(them), Adoption.KIND, bytes)
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
            subnets().firstNotNullOfOrNull { it.ownAlias?.takeIf { a -> a.isNotBlank() } }
                ?: defaultAlias()
                ?: hex(p.me()).take(8),
        )
        Thread { if (settled != null) a.settle(settled, label) else a.begin() }.start()
    }

    /** The kernel calls an [Adoption] makes, bracketed like every other. */
    private fun adoptionKern(p: Participant): Adoption.Kern = object : Adoption.Kern {
        override fun freshSeries(): Int? =
            call("freshSeries", { null }) { p.freshSeries((1u..16u).toList()) }?.toInt()

        override fun backPointers(): List<ByteArray> =
            call("backPointers", { emptyList() }) { p.backPointers() }

        override fun carrying(): String? =
            meet?.carrying()?.id?.takeIf { it.length == 64 }

        override fun position(): String = hex(p.position())

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
            val into = meet?.scope()?.id?.takeIf { it.length == 64 }?.let { unhex(it) }
            p.proposeAdoption(into, unhex(node), unhex(presence), series.toUInt(), nodeBack, carrying?.let { unhex(it) })
        }

        override fun reviewAndSign(body: ByteArray): Adoption.Result<ByteArray> =
            result("reviewAndSignAdoption") { p.reviewAndSignAdoption(body) }

        override fun envelope(
            body: ByteArray,
            entries: List<Pair<String, ByteArray>>,
        ): Adoption.Result<ByteArray> = result("adoptionEnvelope") {
            val mine = p.signBody(body)
            val all = entries.map { (who, e) -> uniffi.rhtn_ffi.SignedEntries(unhex(who), e) } +
                uniffi.rhtn_ffi.SignedEntries(p.me(), mine)
            uniffi.rhtn_ffi.adoptionEnvelope(body, all)
        }

        override fun take(envelope: ByteArray): Adoption.Result<String> =
            result("takeAdoption") { hex(p.takeAdoption(envelope)) }

        override fun positionIn(anchor: String): String? =
            call("positionIn", { null }) { p.positionIn(unhex(anchor)) }?.let { hex(it) }
    }

    /** One kernel call whose refusal is the adoption's to carry. */
    private inline fun <T> result(method: String, body: () -> T): Adoption.Result<T> =
        call(method, { Adoption.Result.Refused(it.reason) }) { Adoption.Result.Ok(body()) }

    // ---- A3, the peering the meeting carried ---------------------------

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
    fun peeringBegins() {
        val p = participant ?: return
        val m = meet ?: return
        if (!m.peeringAsked()) return
        val txid = m.recordTxid() ?: return
        val them = m.counterpartyKey ?: return
        if (peering != null) return
        val pr = Peering(
            me = hex(p.me()),
            counterparty = them,
            counterpartyName = m.counterpartyName,
            presence = txid,
            kern = peeringKern(p),
            post = { bytes ->
                call("send", { e -> Diag.warn("peer.send", "refused" to e.reason); false }) {
                    p.send(unhex(them), Peering.KIND, bytes)
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
            call("backPointers", { emptyList() }) { p.backPointers() } ?: emptyList()

        override fun propose(
            other: String,
            presence: String,
            otherBack: List<ByteArray>,
        ): Peering.Result<ByteArray> = peered("proposePeering") {
            // no replication commitment: `wire-format.md` §4.4 field 6 is
            // optional and what it promises is an operator's to offer
            p.proposePeering(unhex(other), unhex(presence), otherBack, null)
        }

        override fun reviewAndSign(body: ByteArray): Peering.Result<ByteArray> =
            peered("reviewAndSignPeering") { p.reviewAndSignPeering(body) }

        override fun envelope(
            body: ByteArray,
            entries: List<Pair<String, ByteArray>>,
        ): Peering.Result<ByteArray> = peered("peeringEnvelope") {
            val mine = p.signBody(body)
            val all = entries.map { (who, e) -> uniffi.rhtn_ffi.SignedEntries(unhex(who), e) } +
                uniffi.rhtn_ffi.SignedEntries(p.me(), mine)
            uniffi.rhtn_ffi.peeringEnvelope(body, all)
        }

        override fun take(envelope: ByteArray): Peering.Result<String> =
            peered("takePeering") { hex(p.takePeering(envelope)) }
    }

    /** One kernel call whose refusal is the peering's to carry. */
    private inline fun <T> peered(method: String, body: () -> T): Peering.Result<T> =
        call(method, { Peering.Result.Refused(it.reason) }) { Peering.Result.Ok(body()) }

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
        // which subnet, and what this device calls itself and the subnet
        // there, as its claim: the scope chosen as patron, or the subnet
        // this device acts in otherwise, so a regular meeting carries a
        // name too (`light-client-requirements.md` §7)
        val scope = m.scope() ?: subnets().firstOrNull { it.id.hex() == subnet() }?.let { Meet.Scope.of(it) }
        // **and the name, so it is visible from the scan** [author,
        // 2026-10-29]: the one published there, else the default
        val alias = scope?.ownAlias?.takeIf { it.isNotBlank() } ?: defaultAlias()
        return Bootstrap.encode(me, kind, m.kind.peering, scope, alias)
    }

    /** The alias a contact published, where one has reached this device. */
    fun aliasOf(keyHex: String): String? {
        val p = participant ?: return null
        return aliases(p)[keyHex]
    }

    /**
     * **Fetch what every located contact published** (`wire-format.md`
     * §7.11): from wherever each locator leads, kept beside the contact
     * and shown by [People]. Whatever does not answer leaves the keyhash
     * standing.
     */
    fun requestProfiles() {
        val p = participant ?: return
        call("requestProfiles", { }) { p.requestProfiles() }
    }


    /**
     * **What a client may show of a resource's answer**
     * (`light-client-requirements.md` §8): the kernel's reading of the
     * declared type, never this shell's guess at the body.
     */
    enum class Shape { PAGE, TEXT, OTHER }

    /** A resource's answer, as the kernel read the one HTTP message. */
    data class Page(
        val httpStatus: Int,
        /** The declared media type, or null where none was declared. */
        val media: String?,
        val charset: String?,
        val shape: Shape,
        val body: ByteArray,
    )

    /**
     * What a resource answered: §11.1's status, the page where one was
     * delivered **and could be read**, and why not where it could not.
     *
     * **This shell never sees the HTTP message.** `wire-format.md` §11.2
     * names what a second parser costs on the node's side of this
     * exchange, and the same hazard sits on this side: the kernel reads
     * it once and hands up what it found.
     */
    data class Answered(
        val status: ULong,
        val page: Page?,
        val unreadable: String?,
        /** **A typed ask of this client**, where the resource answered
         *  with one instead of a page (`light-client-requirements.md`
         *  §8.1): read by the kernel, put to the person by the shell. */
        val ask: Ask? = null,
    )

    /**
     * **What a resource asked of this client**, in the resource's own
     * words where they are its, and in the kernel's reading otherwise.
     * The resource and node are the ones the answer goes to; this shell
     * shows them and cannot change them.
     */
    data class Ask(
        val id: ULong,
        /** `scan`, `capture`, `location`, `check-in`, `tag`, `sign`,
         *  `live`, `notify`, `export`, `import`. */
        val utility: String,
        val purpose: String,
        val retention: String,
        val resource: String,
        val node: String,
        /** `sign`: the exact bytes to be approved. */
        val payload: ByteArray?,
        /** `location`: the geohash length asked for. */
        val precision: Int?,
        val media: List<String>,
        val channel: String?,
        val seconds: ULong?,
    )

    private fun answered(a: uniffi.rhtn_ffi.ResourceAnswer): Answered = Answered(
        status = a.status,
        page = a.page?.let { q ->
            Page(
                httpStatus = q.httpStatus.toInt(),
                media = q.media,
                charset = q.charset,
                shape = when (q.shape) {
                    uniffi.rhtn_ffi.PageShape.PAGE -> Shape.PAGE
                    uniffi.rhtn_ffi.PageShape.TEXT -> Shape.TEXT
                    uniffi.rhtn_ffi.PageShape.OTHER -> Shape.OTHER
                },
                body = q.body,
            )
        },
        unreadable = a.unreadable,
        ask = a.ask?.let { k ->
            Ask(
                id = k.id,
                utility = k.utility,
                purpose = k.purpose,
                retention = k.retention,
                resource = k.resource.hex(),
                node = k.node.hex(),
                payload = k.payload,
                precision = k.precision?.toInt(),
                media = k.media,
                channel = k.channel,
                seconds = k.seconds,
            )
        },
    )

    /**
     * **Answer an ask** (`light-client-requirements.md` §8.1): the kernel
     * checks the outcome against the ask, signs where the utility signs,
     * composes the one request and sends it to the resource that asked.
     * What comes back is that resource's next answer.
     */
    fun answerUtility(id: ULong, outcome: uniffi.rhtn_ffi.UtilityOutcome): Answered? {
        val p = participant ?: return null
        return call("answerUtility", { null }) { answered(p.answerUtility(id, outcome)) }
    }

    /**
     * **The `authorized_keys` line a launch installs on a machine this
     * identity rents** (`light-client-requirements.md` §10): derived from
     * the signing seed, so a device restored from the phrase derives the
     * same one and the operator's shell comes back with their identity.
     */
    fun hostControlPublicKey(): String? {
        val p = participant ?: return null
        return call("hostControlPublicKey", { null }) { p.hostControlPublicKey() }
    }

    /**
     * **The private half, in the form `ssh -i` reads.** A secret: whoever
     * holds it has a shell on the machines this identity launched. Shown
     * to the person, never written by this shell, and derived again
     * wherever the identity is.
     */
    fun hostControlPrivateKey(): String? {
        val p = participant ?: return null
        return call("hostControlPrivateKey", { null }) { p.hostControlPrivateKey() }
    }

    /**
     * **The `authorized_keys` entry for the key this identity hands its
     * own instance**: bound by the host to the package runner and nothing
     * else, so installing a resource is an act through this client rather
     * than a shell on the machine.
     */
    fun hostRunnerAuthorizedLine(): String? {
        val p = participant ?: return null
        return call("hostRunnerAuthorizedLine", { null }) { p.hostRunnerAuthorizedLine() }
    }

    /**
     * **The private half of the runner's key**, which goes to the
     * instance with a request and is kept there no longer than the act.
     * Never the operator's own key, which opens a shell.
     */
    fun hostRunnerPrivateKey(): String? {
        val p = participant ?: return null
        return call("hostRunnerPrivateKey", { null }) { p.hostRunnerPrivateKey() }
    }

    /** Sweep the serving node's catalog (`wire-format.md` §6.4). */
    fun browse(): String? {
        // **every held identity sweeps its own** [author, 2026-11-07]: a
        // catalog is read inside the subnets an identity belongs to, and
        // the roll below shows them together. The refusal reported is the
        // first, since one identity's node refusing says nothing about
        // another's.
        if (loaded.isEmpty()) return "the kernel is not running"
        var why: String? = null
        for ((_, p) in loaded) {
            call("browse", { if (why == null) why = it.reason }) { p.browse() }
        }
        return why
    }


    /**
     * **Ask a resource for something** (`wire-format.md` §11.1), through
     * the node that served its entry. The message is an HTTP/1.1 request
     * and this shell composes the simplest one there is; what the resource
     * answers is the application's, and nothing is kept of it.
     */
    fun ask(node: String, resource: String, path: String): Answered? {
        val me = activeId ?: return null
        return askAs(me, node, resource, path)
    }

    /**
     * **Reach a resource as the identity that holds its catalog entry**
     * [author, 2026-11-07]: a resource is reached inside a subnet, which
     * is acting within one — so the entry's holder is who asks, and not
     * whichever identity happens to act ([Identities.Owned]).
     */
    fun askAs(by: String, node: String, resource: String, path: String): Answered? {
        val p = asHolder(by) ?: return null
        val request = ("GET $path HTTP/1.1\r\nhost: resource\r\n\r\n").toByteArray()
        return call("requestResource", { null }) {
            answered(p.requestResource(unhex(node), unhex(resource), request))
        }
    }


    /** The subnets this device holds a position in, labelled. */
    /**
     * **This person's own name for the foot, and their keyhash.**
     *
     * **There is no default alias at the network level** [found
     * 2026-10-28]: `wire-format.md` §7.11 profiles are published *per
     * subnet* — field 2 is the subnet and kind 0 is the subject's own
     * profile there — so a name that holds everywhere is a shell notion
     * and not an object. The author's ruling of 2026-10-27 is one
     * default made at creation with a per-subnet override; until the
     * shell holds that default, this answers with the first per-subnet
     * name this key has published and the keyhash where it has published
     * none.
     */
    /**
     * **This identity's keyhash**, which is who an install's resource is
     * owned by (`infra-client-requirements.md` §9.4): the owner whose
     * horizon §11's membership gate is evaluated against.
     */
    fun keyhash(): String? {
        val p = participant ?: return null
        return call("me", { null }) { hex(p.me()) }
    }

    /**
     * **A fresh identity for a resource, of which only the keyhash is
     * kept** (`fresh_resource_keyhash`): an offering cannot carry one and
     * the operator should not be typing one.
     */
    fun mintResource(): String? =
        runCatching { uniffi.rhtn_ffi.freshResourceKeyhash() }.getOrNull()

    fun ownAlias(): Pair<String?, String> {
        val p = participant ?: return null to ""
        val me = call("me", { "" }) { hex(p.me()) }
        // a subnet's own name for this person first, then the default the
        // foot carries [author, 2026-10-27]
        val named = subnets().firstNotNullOfOrNull { it.ownAlias?.takeIf { a -> a.isNotBlank() } }
            ?: call("defaultAlias", { null }) { p.defaultAlias() }
        return named to me
    }




    /** The subnet this device acts in, hex, where it is in one. */
    fun subnet(): String? {
        val p = participant ?: return null
        return call("subnet", { null }) { p.subnet()?.let { hex(it) } }
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
    /**
     * **The optical exchange's trace to this side's nominees** (kind 21)
     * [author, 2026-10-19], one message per event and never batched: the
     * flow drains the exchange as each read lands and hands the events
     * here, where each goes to the nominees this side engaged.
     *
     * **A failure is not the meeting's end.** A nominee that cannot be
     * reached is a witness that does not appear (design §7.1); what the
     * trace buys is the arrival pattern on the clocks of the nominees it
     * does reach, and the exchange itself is unaffected.
     */
    fun opticalTrace(events: List<OpticalExchange.Event>) {
        if (events.isEmpty()) return
        val p = participant ?: return
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
                courier(p).trace(e)?.let { why ->
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

    /** The one thread the trace goes out on, in order (see [opticalTrace]). */
    private val traceWork: java.util.concurrent.ExecutorService =
        java.util.concurrent.Executors.newSingleThreadExecutor { r ->
            Thread(r, "fueros-trace").apply { isDaemon = true }
        }

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
                adopt(who)
                opticalTaken = true
                m.note("their contribution is in; showing the meeting id.")
                null
            }
        }
        if (transcriptTaken) return null
        var refused: String? = null
        val id = call("takeTranscript", { refused = it.reason; null }) { p.takeTranscript(bytes) }
            ?: return refused ?: "the transcript was refused"
        transcriptTaken = true
        m.note("the two meeting ids agree: ${hex(id).take(16)}…")
        // THE BEARER'S TURN. The optical step is what the integrity
        // rests on (§14.3.1); from here a radio carries the bulk,
        // and everything it carries is checked against what the
        // screens showed.
        carryIntent(m, id)
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
    /** How often the turn heartbeat repeats an unchanged state, so that a
     *  loop which is alive but idle is distinguishable from one that has
     *  stopped. Five seconds: frequent enough that a tester waiting on a
     *  screen sees it move, rare enough not to be the chatter a client
     *  review already objected to [2026-10-11]. */
    private val turnDiagMs = 5_000L

    @Volatile private var lastTurnState = ""

    @Volatile private var lastTurnAt = 0L

    @Volatile private var lastEntryAt = 0L

    /** **That the loop is about to ask for an event**, on the clock. With
     *  [turnHeartbeat] this brackets the one call in the loop that can
     *  block: an entry with no heartbeat after it is a park in
     *  `nextEvent`, and no entry at all is a loop that is not running. */
    private fun turnEntry(turns: Long) {
        if (!Diag.enabled()) return
        val now = System.currentTimeMillis()
        if (now - lastEntryAt < turnDiagMs) return
        lastEntryAt = now
        Diag.event("kernel.turn", "turns" to turns)
        // the heap beside it, on the same clock: a number that climbs across
        // a run is a leak, and one that does not rules it out [bench,
        // 2026-10-24]
        val rt = Runtime.getRuntime()
        Diag.event(
            "shell.memory",
            "java_kb" to (rt.totalMemory() - rt.freeMemory()) / 1024,
            "native_kb" to android.os.Debug.getNativeHeapAllocatedSize() / 1024,
        )
    }

    /** [Meet.turnState] into the diagnostics, on change or on the clock;
     *  `meet=null` where there is no meeting, so that a turn always says
     *  something and silence can only mean the loop. */
    private fun turnHeartbeat(m: Meet?) {
        if (!Diag.enabled()) return
        val now = System.currentTimeMillis()
        val s = m?.turnState(now) ?: "meet=null"
        if (s == lastTurnState && now - lastTurnAt < turnDiagMs) return
        lastTurnState = s
        lastTurnAt = now
        Diag.event("meet.turn", "state" to s)
    }

    private fun courier(p: Participant): Meet.Courier = object : Meet.Courier {
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
            val name = defaultAlias() ?: return null
            return call("send", { it.reason }) {
                p.send(unhex(them), Adoption.KIND, Adoption.Frame.hello(name))
                null
            }
        }
        override fun queries(): String? = refusal("converseQueries") { p.converseQueries() }.also { sendCarriages(p) }
        override fun gathered(): String? = refusal("converseGathered") { p.converseGathered() }.also { sendCarriages(p) }
        override fun propose(): String? = refusal("conversePropose") { p.conversePropose() }.also { sendCarriages(p) }
        override fun progress(): Meet.Progress? =
            call("progress", { null }) { p.progress() }?.let { pr ->
                Meet.Progress(
                    proposer = pr.proposer,
                    queriesOutstanding = pr.queriesOutstanding.toInt(),
                    witnessesOutstanding = pr.witnessesOutstanding.toInt(),
                    opened = pr.opened,
                    handed = pr.handed,
                    proposable = pr.proposable,
                    noWitness = pr.noWitness,
                    waitingOn = pr.waitingOn,
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
     * Enter the conversation as the window opens: the nominees and the
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
        is Conversation.Finalized -> {
            // nothing more is met over this bearer: stop offering, so a
            // later scanner in the room does not find this phone first
            ble?.quiet()
            // who the record says witnessed it, from the signed body
            val named: List<List<ByteArray>> = participant?.recordWitnesses(c.txid) ?: listOf()
            val witnesses: List<String> = named.mapNotNull { pair: List<ByteArray> ->
                pair.firstOrNull()?.let { key: ByteArray -> hex(key) }
            }
            Meet.Turn.Finalized(hex(c.txid), witnesses)
        }
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
        shell?.endCapture()
    }

    /**
     * **The counterparty, adopted as this device's lasting knowledge** at
     * the point its key is pinned — which both sides do at the optical
     * step, alike [author, 2026-10-08]: showing the invitation against
     * scanning it is an incident of the world and not a fact the rest of
     * the ceremony carries. Before this the provision had to name whoever
     * a device could meet; now the meeting is what introduces them
     * (`wire-format.md` §14.3.1, design §12.3), and the provisioned peer
     * remains only for messaging before any meeting.
     */
    private fun adopt(who: ByteArray) {
        val key = hex(who)
        synchronized(lock) {
            peer = who
            peerKey = key
            if (peerName.isEmpty() || peerName == "the other person") peerName = key.take(8)
        }
        front.peerKnown(key, peerName)
    }

    /** Who this device is talking to: the meeting's counterparty while one
     *  is open, the adopted or provisioned peer otherwise. */
    private fun counterparty(): ByteArray? =
        meet?.counterpartyKey?.let { runCatching { unhex(it) }.getOrNull() } ?: peer

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
     * the bearer to the shell, so this uses the invitation's own asymmetry:
     * the party that showed it offers, the party that read it seeks.
     */
    private fun carryIntent(m: Meet, ceremony: ByteArray) {
        val p = participant ?: return
        val radio = BleBearer(appContext ?: return)
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
                // the capture starts once theirs is in as well
                m.intentSent()
                // **and this side's candidates behind it** (design §12.6.3):
                // the route, anchored, so the two leave with a path to each
                // other that no node carried — the one the conversation's
                // legs and an adoption after a nodeless record take. Not
                // fatal: with nothing to offer the relay stands, and it is said
                call("candidateOffer", { e -> m.note("no candidates offered: ${e.reason}"); null }) {
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
        val out = call("carriages", { listOf() }) { p.carriages() }
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
        if (carriedPending.getAndSet(false)) m.poll(courier(p))
    }

    /** Whether [drainBearerLocked] took a conversation carriage that the
     *  flow has not yet been polled for. */
    private val carriedPending = java.util.concurrent.atomic.AtomicBoolean(false)

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
                call("takePrekeyCarriage", { e -> m.note("their prekey handover was refused: ${e.reason}") }) {
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
        if (m.step() == Meet.Step.WINDOW && m.openWindow(courier(p)) &&
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
                call("takeCandidateOffer", { e -> m.note("their candidates were refused: ${e.reason}") }) {
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
                    val k = call("takeCaptureKeyCarriage", { e -> m.stop("the capture could not be sealed: ${e.reason}"); null }) {
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
                call("takeIntentCarriage", { e -> m.stop("the carried intent was refused: ${e.reason}") }) {
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
                        call("takeCarriage", { e -> m.note("a carriage was refused: ${e.reason}") }) {
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
        val p = participant ?: return
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
        m.note("my capture key is sent; capturing the counterparty across the window.")
        // **and the prekeys behind it** (`wire-format.md` §14.3.4): this
        // device's signed bundle and one one-time key, so the two can open
        // a channel afterwards with no node to ask. Not the record's
        // concern: a handover that will not cross is noted, and the meeting
        // goes on
        val prekeys = call("prekeyCarriage", { e -> m.note("no prekey handover: ${e.reason}"); null }) {
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
            val turn = call("captureTick", { lost(it); null }) { p.captureTick() } ?: return
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
                call("captureSeal", { e -> m.stop("the capture could not be sealed: ${e.reason}"); false }) {
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
        if (sealed) m.poll(courier(p))
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

    /** The peer this device talks to — adopted at a meeting or named by the
     *  provision — or null: what a conversation screen opens onto. */
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
        // the main thread's watchdog, in a build that has somewhere to say it
        if (Diag.enabled()) Stall.start()
    }

    /**
     * Keep a provision blob for the kernel to read when it comes up.
     *
     * Written before [start] on a cold launch, which is where a provision
     * arrives. **A kernel already up does not re-read it**: it is attached
     * where it was told to attach, and the screen says so rather than
     * appearing to have taken an instruction it did not.
     */
    /**
     * **Serve from this node from now on**, which is the whole of attaching
     * [author, 2026-11-04: "attachment is not a separate act. The operator
     * should attach upon launch and install, other horizon members should
     * agree to attach once when invited and then all horizon members
     * should re-attach every time they become detached."].
     *
     * Two things, because attaching once is not what was asked for. The
     * node is **remembered in the provision**, which is what [bringUp]
     * attaches from at every start — so a device that becomes detached
     * comes back to the same node without being asked again. And it is
     * **attached to now**, because a provision alone takes effect only at
     * the next start and the operator who just launched a machine is
     * entitled to be on it.
     *
     * **Nobody is asked for a key or an address.** The operator's own node
     * speaks as its operator, so the key is this device's own keyhash; the
     * address is the machine this device launched. A member learns both
     * from the invitation it agreed to. There is nothing here a person
     * could usefully type.
     */
    fun serveFrom(context: Context, nodeHex: String, address: String) {
        val shell = AndroidShell(context.applicationContext)
        // the rest of the provision is kept: `known` is what this device
        // was told about other parties, and losing it to an attach would
        // cost it the population it sweeps at start
        val held = runCatching {
            shell.read("provision")?.let { org.json.JSONObject(String(it)) }
        }.getOrNull() ?: org.json.JSONObject()
        held.put("node", nodeHex)
        held.put("addr", address)
        runCatching { shell.write("provision", held.toString().toByteArray()) }
            .onFailure { front.note("· the node could not be remembered: ${it.message}") }
        attachTo(nodeHex, address)
    }

    fun provision(context: Context, blob: String) {
        AndroidShell(context.applicationContext).write("provision", blob.toByteArray())
        synchronized(lock) {
            if (!started || provisioned) return
        }
        front.note("· provision stored. Restart the app for it to take effect.")
    }

    /**
     * Send application bytes **to the party whose thread this is**, named
     * by its keyhash in hex [2026-10-27].
     *
     * The bubble goes up at once as *sending*; the kernel's own send blocks
     * on the network, so it runs off the caller's thread and settles the
     * bubble to *sent* — accepted for carriage, never *delivered* — or to
     * *unsent* with the refusal's reason.
     *
     * **The recipient was this kernel's one `peer` slot until now**, which
     * every meeting overwrote: a conversation screen drew the thread it was
     * opened on and then sent to whoever had been met last, putting the
     * bubble in that other thread as it went. A screen showing one party
     * and sending to another is a misdelivery, not a missing feature, so
     * the recipient is the screen's own and this kernel keeps no current
     * one. Who may be sent to is every party [adopt] has pinned, which is
     * every meeting this device has reached the optical step of.
     */
    fun send(to: String, text: String) {
        val p = participant
        val who = runCatching { unhex(to) }.getOrNull()
        if (p == null || who == null || who.size != 32) {
            front.note(
                if (p == null) "· no kernel yet" else "· not a party to send to: ${to.take(8)}…",
            )
            return
        }
        val id = front.outgoing(to, text)
        Thread({
            call("send", { e -> front.settle(to, id, Front.Delivery.UNSENT, e.reason) }) {
                p.send(who, kindApplication(), text.toByteArray())
                front.settle(to, id, Front.Delivery.SENT, null)
            }
        }, "fueros-send").start()
    }

    /**
     * **The network is the platform's to know** (`light-client-requirements.md`
     * §9: the hardware crosses inward). The default network coming up —
     * at start, and again after it was lost — is handed to the kernel,
     * which stands its direct socket again where the device now routes
     * out of another interface and reaches for every peer (design §6.3).
     * The kernel reaches for them on its own clock besides; this is the
     * reconnect case, which a clock alone would meet late. Off the
     * callback's thread: a round dials, and a dial waits.
     */
    private fun watchNetwork(p: Participant) {
        val cm = appContext?.getSystemService(android.net.ConnectivityManager::class.java) ?: return
        try {
            cm.registerDefaultNetworkCallback(object : android.net.ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: android.net.Network) {
                    Diag.event("shell.network", "state" to "available")
                    Thread({
                        call("network_changed", { _ -> }) {
                            val round = p.networkChanged()
                            Diag.event(
                                "peer.round",
                                "peers" to round.size,
                                "held" to round.count { it.held },
                                "how" to round.joinToString(",") { it.how },
                            )
                            round.forEach { c ->
                                val who = hex(c.who).take(8)
                                if (c.how == "dialled") front.note("· peer $who… reached directly")
                            }
                        }
                    }, "fueros-network").start()
                }

                override fun onLost(network: android.net.Network) {
                    Diag.event("shell.network", "state" to "lost")
                }
            })
        } catch (e: Exception) {
            Diag.warn("shell.network", "state" to "unwatched", "error" to (e.message ?: e.javaClass.simpleName))
        }
    }

    /**
     * **The bench's two levers on the peer exchange** (fieldtest flavour,
     * by intent extra): forget one contact, so that a peer's copy can be
     * watched bringing it back; and reach for the peers now rather than
     * at the clock's next tick. Each runs off the caller's thread and says
     * what it did on the home screen's notes.
     */
    fun forgetContact(hexKey: String) {
        val p = participant ?: run { front.note("· no kernel yet"); return }
        val who = runCatching { unhex(hexKey) }.getOrNull() ?: run { front.note("· not a key: $hexKey"); return }
        Thread({
            call("forget_contact", { e -> front.note("· forget refused: ${e.reason}") }) {
                val had = p.forgetContact(who)
                front.note(if (had) "· forgot contact ${hexKey.take(8)}…" else "· no such contact ${hexKey.take(8)}…")
            }
        }, "fueros-forget").start()
    }

    /** Every contact out, for the same watch; how many went is noted. */
    fun forgetContacts() {
        val p = participant ?: run { front.note("· no kernel yet"); return }
        Thread({
            call("forget_contacts", { e -> front.note("· forget refused: ${e.reason}") }) {
                val n = p.forgetContacts()
                front.note("· forgot $n contact(s)")
            }
        }, "fueros-forget").start()
    }

    /**
     * **The provider credentials, kept by the kernel** (`light-client-
     * requirements.md` §10): one JSON document, providers by name, which
     * the backup carries under the identity. Read and written whole; the
     * kernel keeps the bytes and reads none of them.
     */
    fun providerCredentials(): Map<String, String> {
        val p = participant ?: return emptyMap()
        val bytes = call("provider_credentials", { null }) { p.providerCredentials() } ?: return emptyMap()
        return runCatching {
            Json.obj(Json.parse(String(bytes))).mapNotNull { (k, v) -> Json.string(v)?.let { k to it } }.toMap()
        }.getOrDefault(emptyMap())
    }

    fun holdProviderCredential(provider: String, credential: String): String? {
        val p = participant ?: return "the kernel is not running"
        val all = providerCredentials().toMutableMap()
        if (credential.isBlank()) all.remove(provider) else all[provider] = credential
        val doc = "{" + all.entries.joinToString(",") { (k, v) -> "${Json.quote(k)}:${Json.quote(v)}" } + "}"
        return call("hold_provider_credentials", { e -> e.reason }) {
            p.holdProviderCredentials(if (all.isEmpty()) ByteArray(0) else doc.toByteArray())
            null
        }
    }

    /** How crowded a provider already is inside this operator's own
     *  horizon (PRD-11): the count, and whether it is marked. */
    fun crowded(asn: Long): Pair<Int, Boolean> {
        val p = participant ?: return 0 to false
        val rows = call("providers_by_concentration", { emptyList() }) {
            p.providersByConcentration(listOf(asn.toULong()))
        }
        val row = rows.firstOrNull() ?: return 0 to false
        return row.nodes.toInt() to row.crowded
    }

    /** **How crowded each of a provider's zones already is inside this
     *  operator's own horizon** (PRD-11), keyed by the pair: dispersal by
     *  ASN alone remediates the provider, and by ASN and zone together
     *  also what attaches to a place (design §3.4). One call for the whole
     *  list, a region picker holding a dozen of them. */
    fun crowdedZones(asn: Long, zones: List<String>): Map<String, Pair<Int, Boolean>> {
        val p = participant ?: return emptyMap()
        if (zones.isEmpty()) return emptyMap()
        val rows = call("zones_by_concentration", { emptyList() }) {
            p.zonesByConcentration(asn.toULong(), zones)
        }
        return rows.associate { it.zone to (it.nodes.toInt() to it.crowded) }
    }

    /** This device's public key material, hex: what an instance is told
     *  to speak as. */
    fun materialHex(): String? = participant?.material()?.joinToString("") { "%02x".format(it) }

    // ---- the device's own attach ---------------------------------------



    /**
     * **Attach to a serving node the person named** [2026-10-27].
     *
     * Until this, the only attach was the one `bringUp` performs from the
     * bench's provision blob, read once at a cold start — so an installed
     * app had no way to reach a node at all, and every run that needed one
     * was a run a script had set up. A second attach replaces the first,
     * which is what returning to a patron after a spell on a sibling is.
     *
     * **The population swept is the parties this device holds**, not the
     * nodes: a sweep stocks payload material for people, and a node is
     * where they are reached rather than somebody to reach.
     *
     * Off the caller's thread: an attach dials, publishes and sweeps.
     */
    fun attachTo(nodeHex: String, address: String) {
        val p = participant ?: run { front.note("· no kernel yet"); return }
        val node = Attach.node(nodeHex)?.let { runCatching { unhex(it) }.getOrNull() }
        val at = Attach.address(address)
        if (node == null || at == null) {
            front.note("· not a node and an address: ${nodeHex.take(8)}… at $address")
            return
        }
        Thread({
            val population = call("contacts", { emptyList() }) {
                // the people, not the infrastructure: `how` 4 is a node
                p.contacts().filter { it.how != How.INFRA }.map { it.keyhash }
            }
            val a = call("attach", { e -> front.note("· attach refused: ${e.reason}"); null }) {
                p.attach(node, listOf(at), population)
            }
            if (a == null) {
                // **a node that did not answer is the detached case**, as
                // it is on the provision's path: the loop goes on running
                front.note("· $at did not answer: this device stays detached.")
                show(p, Connection.Detached)
            } else {
                // **not the provision's flag**: that one says a blob was
                // read, and this attach is the person's own
                front.note("· attached to ${hex(node).take(8)}… at $at; ${a.queued} queued there.")
                show(p, Connection.Attached(node, a.primary))
            }
        }, "fueros-attach").start()
    }

    /**
     * **Detach, which the person may do and the screen offers**: the
     * session closes cleanly so the serving node queues for this device
     * from now rather than after three missed intervals. Everything else
     * is kept.
     */
    fun detachNow() {
        val p = participant ?: run { front.note("· no kernel yet"); return }
        Thread({
            call("detach", { e -> front.note("· detach refused: ${e.reason}") }) {
                p.detach()
                front.note("· detached. Nothing is queued for this device from here.")
                show(p, Connection.Detached)
            }
        }, "fueros-detach").start()
    }

    // ---- authority: the lines this key holds, and what ending one costs






    /**
     * **The aliases this device has been told**, by keyhash: what the
     * contacts list holds, which is where a published profile lands.
     * Absent for a party that has published none to this device, and
     * that absence is shown as the keyhash rather than filled in.
     */
    private fun aliases(p: Participant): Map<String, String> =
        call("contacts", { emptyMap() }) {
            p.contacts().mapNotNull { c -> c.alias?.let { a -> c.keyhash.hex() to a } }.toMap()
        }

    /** The aliases held, by keyhash in hex: what a screen names a party by. */
    fun aliases(): Map<String, String> = participant?.let { aliases(it) } ?: emptyMap()

    // ---- the nouns, as the model holds them --------------------------
    //
    // **Ports, not wrappers** (`Robot/implementation-plan.md` section 8):
    // each hands up the kernel's record as the model defines it — full
    // keyhashes, the wire's enums — and a screen's pure part decides what
    // to show of it. Nothing here shortens, words or renames.

    fun contacts(): List<Contact> {
        val p = participant ?: return emptyList()
        return call("contacts", { emptyList() }) { p.contacts() }
    }

    fun subnets(): List<Subnet> {
        val p = participant ?: return emptyList()
        return call("subnets", { emptyList() }) { p.subnets() }
    }

    /** **An invitation read and its key placed** against what this device
     *  holds (`rhtn_client::invitation`); null where it is not one this
     *  build reads, and [invitationSay] gives the words for that. */
    fun invitation(text: String): uniffi.rhtn_ffi.Invited? {
        val p = participant ?: return null
        return call("invitation", { null }) { p.invitation(text) }
    }

    /** Why an invitation could not be read, in the words a person is told. */
    fun invitationSay(text: String): String =
        runCatching { uniffi.rhtn_ffi.invitationSay(text) }.getOrDefault("it could not be read")

    /** **Every row this device would assert about its horizon** to a node
     *  it operates (`rhtn_client::model::HorizonRow`): open bindings whose
     *  member's key material this device holds, each already in the form
     *  the administration surface reads. */
    fun horizonRows(): List<uniffi.rhtn_ffi.HorizonRow> {
        val p = participant ?: return emptyList()
        return call("horizonRows", { emptyList() }) { p.horizonRows() }
    }

    /**
     * **The number this handset reports for itself, handed to the kernel
     * for the meeting about to run** [author, 2026-11-10]: *"Pull from the
     * handset at meeting time and pass it to the counterparty during the
     * bluetooth session."* It rides the intent over the local bearer and
     * nothing else carries it.
     *
     * **Nullable, and nothing is asked of anybody.** No SIM, a carrier
     * that does not provision the number, or a refused permission all
     * come to the same call with null, and the meeting runs unchanged
     * (`light-client-requirements.md` §1.5).
     */
    fun declareNumber(number: String?) {
        val p = participant ?: return
        call("declareNumber", { }) { p.declareNumber(number) }
    }

    /** The number held for a party, where their device reported one. */
    fun numberHeld(who: String): String? {
        val p = participant ?: return null
        if (!who.isHash32()) return null
        return call("numberHeld", { null }) { p.number(who.unhex()) }
    }

    /**
     * **The invitations this patron owes**, for a node answering at `at`:
     * where to send each and the link to send. Reading is also what drops
     * an address past its subject's window.
     */
    fun invitations(at: String): List<uniffi.rhtn_ffi.Invite> {
        val p = participant ?: return emptyList()
        return call("invitations", { emptyList() }) { p.invitations(at) }
    }

    /**
     * **This device's archive and store, enveloped** (design §13.7.1,
     * `rhtn_client::backup`): what a device loss takes away, under a key
     * derived from the identity, so the blob is unreadable wherever it is
     * kept until the identity is in hand again.
     *
     * The identity is **not** in it: that is the one thing a backup must
     * not carry, and the seed phrase is how it comes back.
     */
    fun exportBackup(): ByteArray? {
        val p = participant ?: return null
        return call("exportBackup", { null }) { p.exportBackup() }
    }

    /** The subnets this device roots and could bring under a patron. */
    fun subnetsRooted(): List<Subnet> {
        val p = participant ?: return emptyList()
        return call("subnetsRooted", { emptyList() }) { p.subnetsRooted() }
    }

    /** The patron lines this key holds, newest series and all. */
    fun lines(): List<Line> {
        val p = participant ?: return emptyList()
        return call("lines", { emptyList() }) { p.lines() }
    }

    /** The meetings this device took part in, newest first, as outcomes. */
    fun meetings(): List<Meeting> {
        val p = participant ?: return emptyList()
        return call("meetings", { emptyList() }) { p.meetings() }
    }

    /** The infra nodes whose endpoint records this device holds. */
    fun instances(): List<Instance> {
        val p = participant ?: return emptyList()
        return call("instances", { emptyList() }) { p.instances() }
    }

    /** What the catalog holds, with each node's portion marked. */
    fun resources(): List<Resource> {
        val p = participant ?: return emptyList()
        return call("resources", { emptyList() }) { p.resources() }
    }

    /**
     * **What leaving `patronHex` costs** (`light-client-requirements.md`
     * §6), or why it could not be said — the kernel's own reason, so a
     * patron this key holds no relationship with reads differently from
     * a kernel that has not come up.
     */
    fun costOfLeaving(patronHex: String): Pair<Cost?, String> {
        val p = participant ?: return null to "the kernel is not up yet"
        val who = runCatching { unhex(patronHex) }.getOrNull()
            ?: return null to "not a key: ${patronHex.take(16)}…"
        return call("cost_of_leaving", { e -> front.note("· cost refused: ${e.reason}"); null to e.reason }) {
            p.costOfLeaving(who) to ""
        }
    }



    /**
     * **Leave a patron**: minted, signed, appended and posted in one act.
     * The old patron does not sign and no refusal can come back from them
     * (design §6.2); what can come back is this client's own, and that is
     * what the returned line says.
     *
     * Blocking, and the caller runs it off the drawing thread.
     */
    fun departFrom(patronHex: String): String {
        val p = participant ?: return "no kernel yet"
        val who = runCatching { unhex(patronHex) }.getOrNull() ?: return "not a key: $patronHex"
        return call("depart", { e -> "refused: ${e.reason}" }) {
            val t = p.depart(who, null)
            front.note("· left ${patronHex.take(8)}…; the departure is ${hex(t).take(8)}…")
            "left ${patronHex.take(8)}… — the departure is ${hex(t).take(8)}…"
        }
    }

    fun forgetCaptures(hexKey: String) {
        val p = participant ?: run { front.note("· no kernel yet"); return }
        val who = runCatching { unhex(hexKey) }.getOrNull() ?: run { front.note("· not a key: $hexKey"); return }
        Thread({
            call("forget_captures", { e -> front.note("· forget refused: ${e.reason}") }) {
                val n = p.forgetCaptures(who)
                front.note("· forgot $n sealed capture(s) of ${hexKey.take(8)}…")
            }
        }, "fueros-forget").start()
    }

    fun contactPeersNow() {
        val p = participant ?: run { front.note("· no kernel yet"); return }
        Thread({
            call("contact_peers", { e -> front.note("· contact refused: ${e.reason}") }) {
                val round = p.contactPeers()
                Diag.event("peer.round", "peers" to round.size, "held" to round.count { it.held }, "how" to round.joinToString(",") { it.how })
                front.note("· peers: " + round.joinToString("; ") { "${hex(it.who).take(8)}… ${it.how}" }.ifEmpty { "none" })
            }
        }, "fueros-contact").start()
    }

    /** Start the participant, attach where provisioned, and pump its events. */
    @Volatile private var shell: AndroidShell? = null

    /** Serializes [drainBearer] against its two caller threads. */
    private val bearerLock = Any()

    /**
     * The participant for the identity whose storage `own` is, or null
     * where it would not open. The rotation logic is per identity: see
     * [openHeld].
     */
    private fun openOn(shell: AndroidShell, known: List<ByteArray>): Participant? {
        // **a mark outranks the seeds** ([Rotation.start]): a rotation that
        // began and may not have finished is performed here, because
        // `Participant.rotating` opens the new identity on the storage it
        // is handed — so between beginning one and sealing its new seeds
        // the archive belongs to a key the seeds do not name, and a start
        // that trusted the seeds would open the wrong identity on the
        // right archive
        val mark = shell.unseal(Rotation.MARK_NEW)
        val p = when (
            Rotation.start(
                shell.unseal(Identities.LEGACY_SEEDS)?.size == 64,
                mark?.size == 64,
            )
        ) {
            is Rotation.Start.Unstarted -> {
                // **one identity's seeds being gone is that identity's
                // fact and not the device's** [2026-11-07]: the device
                // may hold others, and [bringUp] says so once where none
                // of them came up
                Diag.warn("identity", "op" to "open", "error" to "no seeds under this scope")
                return null
            }
            is Rotation.Start.Rotate -> {
                // bound here rather than asserted: the predicate above read
                // the same value, and a `!!` would say this code knows
                // something the compiler cannot check
                val fresh = mark ?: return null
                val prior = shell.unseal(Rotation.MARK_PRIOR)
                if (prior == null || prior.size != 64) {
                    // **half a mark is no mark**: without the prior key
                    // there is no rotation to make (`wire-format.md` §4.1
                    // admits no variant carrying one half), so the mark is
                    // dropped and the device starts as it was
                    shell.forget(Rotation.MARK_NEW)
                    front.note("· a rotation was begun without its prior key and was dropped")
                    call("Participant.start", { e -> front.setStatus("kernel refused: ${e.reason}"); null }) {
                        Participant.start(heldSeeds(shell), known, platformOf(shell))
                    } ?: return null
                } else {
                    val out = call("Participant.rotating", { e ->
                        front.setStatus("the rotation was refused: ${e.reason}"); null
                    }) {
                        Participant.rotating(fresh.copyOf(), prior.copyOf(), null, known, platformOf(shell))
                    }
                    prior.fill(0)
                    if (out == null) return null
                    // **the new seeds become the identity only now**, and
                    // the mark goes after them: cleared first and the
                    // process dying would leave an archive nothing names
                    if (!shell.seal(Identities.LEGACY_SEEDS, fresh)) {
                        front.setStatus("the rotation held but its seeds could not be kept")
                        return null
                    }
                    shell.forget(Rotation.MARK_PRIOR)
                    shell.forget(Rotation.MARK_NEW)
                    fresh.fill(0)
                    Diag.event("rotation.done")
                    front.note("· the rotation completed and this device holds its new key")
                    out
                }
            }
            is Rotation.Start.Ordinary ->
                call("Participant.start", { e -> front.setStatus("kernel refused: ${e.reason}"); null }) {
                    Participant.start(heldSeeds(shell), known, platformOf(shell))
                } ?: return null
        }
        return p
    }

    /** The keyhash a seed pair names, or null where it does not read. */
    private fun idOfSeeds(seeds: ByteArray): String? =
        call("keyhashOfSeeds", { null }) { uniffi.rhtn_ffi.keyhashOfSeeds(seeds) }
            ?.takeIf { Identities.names(it) }

    /**
     * **Open one of the identities this device holds**, on storage of its
     * own ([AndroidShell]'s scope): two kernels over one directory would
     * each overwrite the other's archive.
     *
     * **A mark outranks the seeds** ([Rotation.start]), per identity: a
     * rotation that began and may not have finished is performed here,
     * because `Participant.rotating` opens the new identity on the
     * storage it is handed — so between beginning one and sealing its new
     * seeds the archive belongs to a key the seeds do not name, and a
     * start that trusted the seeds would open the wrong identity on the
     * right archive.
     */
    private fun openHeld(app: Context, h: Identities.Held, known: List<ByteArray>) {
        val own = AndroidShell(app, h.scope)
        val p = openOn(own, known) ?: return
        loaded[h.id] = p
    }

    private fun bringUp(shell: AndroidShell) {
        this.shell = shell
        val provision = shell.read("provision")?.let {
            try { JSONObject(String(it)) } catch (_: Exception) { null }
        }
        synchronized(lock) { provisioned = provision != null }
        front.setProvisioned(provision != null)
        // the identity is this device's own, whether or not anybody has
        // been told about it yet
        // **`opt`, not `get`** [2026-11-04]: `getJSONArray` throws where the
        // key is absent, and the `?.let` below only ever guarded a null
        // provision. A provision naming a node and an address and nothing
        // else — which is what agreeing to an invitation writes
        // ([serveFrom]) — crashed the kernel thread on every start, so two
        // bench phones would not launch at all. A provision is a thing
        // other code writes, and every field of it is optional to this.
        val known = provision?.optJSONArray("known")
            ?.let { a -> (0 until a.length()).map { unhex(a.getString(it)) } }
            ?: listOf()
        // **every identity this device holds, opened at once**
        // ([Identities]; author, 2026-11-07): a key rotation replaces the
        // key one subnet knows and leaves the others live, so a device
        // that has rotated holds a key still usable elsewhere.
        val app = appContext ?: return
        var index = Identities.decode(shell.unseal(Identities.INDEX))
        // **the identity held before this model is brought into the index
        // and nothing is moved**: its scope is the top-level directory it
        // already writes, because relocating an archive would be a
        // destructive step at the one moment there is nothing to fall
        // back on ([Identities.LEGACY_SCOPE])
        val legacy = shell.unseal(Identities.LEGACY_SEEDS)?.takeIf { it.size == 64 }
        val adopt = Identities.migrated(legacy != null, legacy?.let { idOfSeeds(it) }, index)
        if (adopt is Identities.Migration.Adopt) {
            index = index + Identities.Held(adopt.id, Identities.LEGACY_SCOPE, null)
            if (!shell.seal(Identities.INDEX, Identities.encode(index))) {
                front.setStatus("this device's storage would not keep its list of identities")
                return
            }
            Diag.event("identity.adopted")
        }
        if (index.isEmpty()) {
            front.setStatus("no identity on this device")
            return
        }
        for (h in index) {
            openHeld(app, h, known)
        }
        // **one of them acts** ([Identities.active]): the saved choice
        // where it is still held, the first otherwise — a device holding
        // identities always has one, or a meeting would have nothing to
        // offer and no way to say why
        val saved = shell.unseal(Identities.ACTIVE)?.toString(Charsets.UTF_8)?.trim()
        activeId = Identities.active(index.filter { loaded.containsKey(it.id) }, saved)
        // **a departure a rotation owes is owed at every start**, since a
        // process that died between the adoption and the departure left
        // the mark behind ([concludeRotation])
        concludeRotation()
        val p = participant ?: run {
            front.setStatus("no identity on this device came up")
            return
        }
        show(p, Connection.Detached)
        // the shell's copy of the nouns, from a kernel that is up now
        Model.refreshAll()
        watchNetwork(p)
        // **a nominee arrives as an introduction** (`wire-format.md` §2.3):
        // material and a signed locator, taken as a person would take one
        // scanned, so the nominee can be offered to the counterparty's
        // prune by locator. The bench names nominees this way because a
        // phone at genesis has met nobody; material alone left the set
        // empty (run 58a6883-a5-2, 2026-10-24)
        provision?.optJSONArray("introductions")?.let { a ->
            for (k in 0 until a.length()) {
                val bytes = runCatching { unhex(a.getString(k)) }.getOrNull() ?: continue
                call("takeIntroduction", { e -> front.note("· an introduction was refused: ${e.reason}") }) {
                    val who = p.takeIntroduction(bytes)
                    front.note("· introduced: ${hex(who).take(16)}…")
                }
            }
        }

        val material = p.material().joinToString("") { "%02x".format(it) }

        // **An unprovisioned device is the ordinary device** [author,
        // 2026-10-11]: it holds its own identity, knows nobody, is
        // attached to nothing, and meets whoever is in front of it — the
        // event loop below is what drives a ceremony and it runs either
        // way. A provision is the bench's convenience (a peer to message,
        // nominees, a node, a stream) and never the way in. Until this
        // the shell returned here, and no phone ever met without a
        // script having written a blob first.
        if (provision == null) {
            // the material is this device's public identity, shown
            // because nothing else exposes it: no flavour logs it and the
            // Report bundle carries identities as eight characters only
            front.note("· this device's public key material:")
            front.note("    $material")
            front.note("· nobody known, nothing attached: a meeting crosses the bearer alone.")
        }

        // the retention this device declares (design §7.5.1), where the
        // provision names one; the kernel's own default otherwise
        val retention = provision?.optLong("retention_years", 0L) ?: 0L
        if (retention > 0L) {
            call("set_retention_years", { e -> front.note("· retention refused: ${e.reason}") }) {
                p.setRetentionYears(retention.toULong())
            }
        }
        // **the provision need not name anybody.** A device that has met
        // nobody is the ordinary case, and the invitation code is what
        // introduces the first counterparty ([counterparty]); a named peer
        // here is only a convenience for messaging before any meeting.
        val to = provision?.optString("peer", "")?.takeIf { it.isNotEmpty() }
            ?.let { runCatching { unhex(it) }.getOrNull() }
        peer = to
        peerName = provision?.optString("peer_name", "the other person") ?: "the other person"
        nominees = provision?.optJSONArray("nominees")
            ?.let { a -> (0 until a.length()).map { unhex(a.getString(it)) } }
            ?: listOf()
        peerKey = to?.let { hex(it) }
        peerKey?.let { front.peerKnown(it, peerName) }
        // **a provision may name no node, and then this device stays
        // detached** (design §6.4): a pair can meet with nothing reachable
        // but each other, and what they form is the witnessless shape —
        // two identities at their genesis, no witnesses, no responses
        // (`wire-format.md` §3.2 subtype 1). The event loop below still
        // runs, because it is what drives the ceremony; only the attach
        // is skipped [author, 2026-10-07].
        val nodeHex = provision?.optString("node", "") ?: ""
        if (nodeHex.isEmpty()) {
            if (provision != null) {
                front.note("· no node in the provision: this device stays detached,")
                front.note("  and a meeting crosses the bearer alone (design §6.4).")
            }
            show(p, Connection.Detached)
        } else {
            val node = unhex(nodeHex)
            val a = call("attach", { e -> front.setStatus("attach refused: ${e.reason}"); null }) {
                // a device that has met nobody attaches with no peers to
                // follow; the first one is learned at the meeting
                p.attach(node, listOf(provision?.optString("addr", "") ?: ""), listOfNotNull(to))
            }
            // **a node named but unreachable is the detached case, not a
            // reason to stop.** This returned, and the event loop below
            // never ran — which is the author's own rule broken twelve
            // lines above, where a provision naming no node skips the
            // attach and runs the loop anyway [author, 2026-10-07].
            //
            // It cost seven attempts at A6. The phones carried
            // `node=5046da37 addr=192.168.0.8:7447` from an earlier
            // `field-run`, and a nodeless run has nothing listening
            // there, so every one of them left the kernel with no loop:
            // no poll, no staleness watchdog, no heartbeat, and a
            // conversation that received every byte it needed and acted
            // on none of it (2026-10-24).
            if (a == null) {
                front.note("· the node named in the provision did not answer:")
                front.note("  this device stays detached, and a meeting crosses")
                front.note("  the bearer alone (design §6.4).")
                show(p, Connection.Detached)
            } else {
                show(p, Connection.Attached(node, a.primary))
            }
        }
        // **the loop's own first word.** Said once, before any turn, so
        // that a run which never reaches the loop is distinguishable from
        // one whose loop parks in its first call (2026-10-24).
        Diag.event("kernel.loop", "state" to "entered")
        var turns = 0L
        while (true) {
            // **before the call that can park, not after.** The turn
            // heartbeat sits past `nextEvent` and the event handling, so a
            // loop parked in `nextEvent` emits nothing at all and says
            // nothing about where it stopped — which is how A6's fifth and
            // sixth attempts left the question open. `next_event` is
            // wrapped in the diagnostics' plain call, which emits nothing,
            // so this is the only line that can place it. On the clock
            // rather than every turn: a 2 s poll would otherwise write
            // thirty lines a minute for a loop doing nothing.
            turnEntry(turns)
            turns += 1
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
                        is Event.Peer -> Diag.id8(e.from)
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
                    // **the kind says where it goes** (`wire-format.md`
                    // §7.10.1): the adoption's legs are this shell's own
                    // frames on their own number, and everything else is
                    // what a person sent a person
                    val said = if (e.kind == Adoption.KIND) Adoption.Frame.heard(e.bytes) else null
                    if (said != null) {
                        // **a name, self-declared, from the party in front
                        // of you** [author, 2026-10-29]: the meeting's to
                        // show, and nobody else's to take
                        val m = meet
                        if (m != null && m.counterpartyKey == from) m.heard(said)
                    } else if (e.kind == Adoption.KIND) {
                        val a = adoption
                        when {
                            // **a frame that arrives before this side has begun
                            // its own is held, not dropped** [bench,
                            // 2026-10-24]: the other phone reached Done a
                            // second earlier and sent its first leg, and this
                            // side's object did not exist yet (run
                            // 5f415ee-a3-peering-1, the peering's back-pointers)
                            a == null -> synchronized(lock) { earlyAdoption += from to e.bytes }
                            from != a.counterparty -> front.note("· a message of an adoption arrived from somebody who is not party to this one")
                            else -> a.frame(e.bytes)
                        }
                    } else if (e.kind == Peering.KIND) {
                        val pr = peering
                        when {
                            pr == null -> synchronized(lock) { earlyPeering += from to e.bytes }
                            from != pr.counterparty -> front.note("· a message of a peering arrived from somebody who is not party to this one")
                            else -> pr.frame(e.bytes)
                        }
                    } else {
                        front.incoming(from, who, String(e.bytes))
                    }
                }
                is Event.Connection -> {
                    show(p, e.v1)
                    Model.connection(e.v1)
                }
                // **the nouns the kernel changed**: the shell's copy is
                // refreshed for exactly those, and the screens watching
                // them redraw
                is Event.Changed -> Model.refresh(e.nouns.toSet())
                // a verifier answered on the end-to-end path: the kernel
                // took it against the query it issued, and the screen
                // shows the verdict against the verifier it names
                // between peers (design §6.3): what a locator, a deposit, a
                // request or a return came to, in the kernel's words
                is Event.Peer -> {
                    Diag.event("peer.event", "from" to Diag.id8(e.from), "what" to e.what)
                    front.note("· peer: ${e.what}")
                }
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
            // **the heartbeat, outside the null guard on purpose.** Said
            // every turn whatever the flow is doing, because its job is to
            // make silence mean one thing: the loop is not turning. Put
            // inside `meet?.let` first, its silence on A6's fifth attempt
            // had two readings — the loop stopped, or `meet` was null —
            // and separating those is the whole reason it exists
            // (2026-10-24). Said when it changes, and otherwise once every
            // `turnDiagMs`. Diag is a no-op where no sink is installed,
            // which is every releasable build.
            turnHeartbeat(meet)
            meet?.let { m ->
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
                if (m.pollDue()) m.poll(courier(p))
                // **the record is what an adoption rests on**, so D7
                // begins where the ceremony ends; begun once, and nothing
                // where the meeting carried none
                if (t == Meet.Step.DONE) {
                    adoptionBegins()
                    // and the peering the invitation's checkbox asked for,
                    // which is independent of the adoption
                    peeringBegins()
                    // **and the departure a concluded rotation owes**
                    // [author, 2026-11-07]: the old key leaves the subnet
                    // that rotated once the new one is adopted into its
                    // place, so this is the moment it is owed
                    concludeRotation()
                }
                if (m.stepStale()) {
                    m.stop("nothing has moved for ${Meet.STALE_MS / 1000} s at ${m.step().name.lowercase()}; the other phone may have stopped")
                }
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
    private fun show(p: Participant, s: Connection) {
        val key = p.presentedKey().joinToString("") { "%02x".format(it) }.take(16)
        val line = when (s) {
            is Connection.Detached -> "detached"
            is Connection.Attached -> if (s.primary) "attached, primary" else "attached"
            is Connection.Reconnecting -> "reconnecting…"
            is Connection.Lost -> "lost: no serving node reachable"
        }
        // the kernel says which; the words are this shell's
        val construction = when (p.payloadConstruction()) {
            Construction.DOUBLE_RATCHET -> "the Double Ratchet (the floor)"
            Construction.TRIPLE_RATCHET -> "the Triple Ratchet"
        }
        front.setStatus("$line · presents $key…\npayload: $construction")
        // **every thread's chip, not the latest party's** [2026-10-27]:
        // the chip is per peer on the thread list (C1), and reading it off
        // one slot left every other thread's showing whatever it showed
        // when that party was the current one
        for (t in front.threads()) {
            runCatching { unhex(t.peerKey) }.getOrNull()?.let { who ->
                if (who.size == 32) front.directPath(t.peerKey, p.directTo(who))
            }
        }
    }

    private fun unhex(s: String): ByteArray =
        ByteArray(s.length / 2) { ((s[2 * it].digitToInt(16) shl 4) + s[2 * it + 1].digitToInt(16)).toByte() }

    private fun hex(b: ByteArray): String = b.joinToString("") { "%02x".format(it) }

    /**
     * The device's own seeds, as the first run sealed them.
     *
     * **Under the shell's own custody and never around it**: these are the
     * whole of the identity, and they never cross the kernel's storage
     * seam, so the kernel's sealing does not reach them — they are
     * Keystore-wrapped here, exactly as the storage key itself is
     * (`light-client-requirements.md` §9).
     *
     * **Nothing is minted here since 2026-10-29** [author]: the first run
     * is a choice between a new identity and one restored from its
     * phrase, and [createIdentity] is where the seeds arrive. A kernel
     * started with none is a shell that skipped that screen, which is a
     * fault to say rather than paper over.
     */
    private fun heldSeeds(shell: AndroidShell): ByteArray {
        val s = shell.unseal(Identities.LEGACY_SEEDS)
        check(s != null && s.size == 64) { "no identity on this device: the first run was not completed" }
        return s
    }

    /** Whether the first run has been completed on this device. */
    fun hasIdentity(context: Context): Boolean {
        val device = AndroidShell(context.applicationContext)
        // the index, or the identity a device held before there was one
        return Identities.decode(device.unseal(Identities.INDEX)).isNotEmpty() ||
            device.unseal(Identities.LEGACY_SEEDS)?.size == 64
    }

    /**
     * **Take the identity off this device** ([Wipe]; PRD-03): the key,
     * the archive, the store, every sealed capture, the shell's own
     * preference stores, and last the Keystore key that wrapped them, so
     * a byte that survived a delete cannot be read.
     *
     * **The kernel handle goes first**, before anything is deleted: a
     * live `Participant` writes through the shell's custody, and a writer
     * running against a half-deleted directory could put a file back.
     * The caller kills the process afterwards for the same reason — the
     * one guarantee that nothing in flight outlives this.
     *
     * Returns what would **not** go, for [Wipe.left] to say. An empty
     * list is a device holding no identity.
     */
    fun wipe(context: Context): List<String> {
        loaded.clear()
        activeId = null
        meet = null
        val app = context.applicationContext
        val left = mutableListOf<String>()
        if (!clear(java.io.File(app.filesDir, Wipe.KERNEL_DIR))) left += Wipe.KERNEL_DIR
        if (!clear(java.io.File(app.filesDir, Wipe.DIAG_DIR))) left += Wipe.DIAG_DIR
        for (store in Wipe.STORES) {
            val ok = runCatching {
                app.getSharedPreferences(store, Context.MODE_PRIVATE).edit().clear().commit()
            }.getOrDefault(false)
            // **and the file itself**, not only its contents: a cleared
            // store still names itself on disk
            runCatching { app.deleteSharedPreferences(store) }
            if (!ok) left += store
        }
        // **the wrapping key last of all**: while it exists, anything
        // recovered from the storage it wrapped is readable, and once it
        // is gone nothing is (`light-client-requirements.md` §6 gives the
        // same reasoning for a deleted archive window's capture keys)
        val alias = runCatching {
            val ks = java.security.KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            ks.deleteEntry(Wipe.AT_REST_ALIAS)
            !ks.containsAlias(Wipe.AT_REST_ALIAS)
        }.getOrDefault(false)
        if (!alias) left += Wipe.AT_REST_ALIAS
        Diag.event("identity.wiped")
        return left
    }

    /** `dir` and everything under it gone, or false where some of it stays. */
    private fun clear(dir: java.io.File): Boolean {
        if (!dir.exists()) return true
        dir.listFiles()?.forEach { if (it.isDirectory) clear(it) else it.delete() }
        return dir.deleteRecursively() || !dir.exists()
    }

    /**
     * **The first run hands the seeds over and the kernel starts on them**
     * ([FirstRunActivity]): sealed under the shell's custody first, so a
     * process that dies between the two finds them at its next start.
     * False where the storage refused them, and nothing started.
     */
    fun createIdentity(context: Context, seeds: ByteArray): Boolean {
        require(seeds.size == 64) { "the kernel's seeds are 64 bytes" }
        val app = context.applicationContext
        val device = AndroidShell(app)
        val id = idOfSeeds(seeds) ?: return false
        val index = Identities.decode(device.unseal(Identities.INDEX))
        // **adding, not replacing** [author, 2026-11-07]: *"'Add an
        // identity' … is just another trigger for 'Start a new
        // identity'"*, so this door is reached from the first run and
        // again later, and an identity already held stays held.
        if (index.any { it.id == id }) return true
        // **its own storage, named for itself**: two kernels over one
        // directory would each overwrite the other's archive. The
        // top-level directory belongs to whatever identity a device held
        // before this model ([Identities.LEGACY_SCOPE]) and is never
        // taken by a new one.
        val scope = Identities.scopeFor(id)
        if (!AndroidShell(app, scope).seal(Identities.LEGACY_SEEDS, seeds)) return false
        // **the seeds land before the index names them**, so a process
        // that dies between leaves an identity on disk that nothing opens
        // rather than an index naming storage that is not there
        if (!device.seal(Identities.INDEX, Identities.encode(index + Identities.Held(id, scope, null)))) {
            return false
        }
        Diag.event("identity.added")
        if (started) {
            // already running: open it beside the others rather than
            // waiting for a restart, and let it act since a person who
            // just made an identity means to use it
            openHeld(app, Identities.Held(id, scope, null), listOf())
            chooseActive(context, id)
        } else {
            start(context)
        }
        return true
    }

    /**
     * **Every held identity's rows, each carrying whose it is** — the
     * concurrent half of the author's division [2026-11-07]: *"Concurrent
     * for viewing or acting within subnets"*.
     *
     * **The holder is what an action routes by** ([Identities.Owned]): a
     * screen acting on a merged row through whichever identity happened
     * to be active would act as the wrong key, and nothing would fail —
     * the act would succeed under somebody the person did not choose.
     *
     * In the index's order, so the list a person reads does not reshuffle
     * between refreshes.
     */
    private fun <T> acrossHeld(what: String, of: (Participant) -> List<T>): List<Identities.Owned<T>> {
        val app = appContext ?: return listOf()
        return identities(app).flatMap { h ->
            val p = loaded[h.id] ?: return@flatMap listOf<Identities.Owned<T>>()
            call(what, { emptyList<T>() }) { of(p) }.map { Identities.Owned(h.id, it) }
        }
    }

    /** Every identity's subnets, each saying whose it is. */
    fun subnetsHeld(): List<Identities.Owned<Subnet>> = acrossHeld("subnets") { it.subnets() }

    /** Every identity's contacts, each saying whose it is. */
    fun contactsHeld(): List<Identities.Owned<Contact>> = acrossHeld("contacts") { it.contacts() }

    /** Every identity's lines, each saying whose it is. */
    fun linesHeld(): List<Identities.Owned<Line>> = acrossHeld("lines") { it.lines() }

    /** Every identity's resources, each saying whose it is. */
    fun resourcesHeld(): List<Identities.Owned<Resource>> =
        acrossHeld("resources") { it.resources() }

    /** Every identity's meetings, each saying whose it is. */
    fun meetingsHeld(): List<Identities.Owned<Meeting>> =
        acrossHeld("meetings") { it.meetings() }

    /**
     * **Act as the identity that holds a row** rather than as the one that
     * acts ([Identities.Owned]). Null where that identity is not open
     * here, which a caller reports rather than falling back to the active
     * one: falling back is the wrong-key act this exists to prevent.
     */
    private fun asHolder(by: String): Participant? = loaded[by]

    /**
     * **Leave a patron as the identity that holds that line** [author,
     * 2026-11-07]: a departure is acting within a subnet, so it belongs to
     * the identity that is in it and not to whichever one is active.
     */
    fun departAs(by: String, patronHex: String): String {
        val p = asHolder(by) ?: return "that identity is not open on this device"
        val who = runCatching { unhex(patronHex) }.getOrNull() ?: return "not a key: $patronHex"
        return call("depart", { e -> "refused: ${e.reason}" }) {
            val t = p.depart(who, null)
            front.note("· ${by.take(8)}… left ${patronHex.take(8)}…; the departure is ${hex(t).take(8)}…")
            "left ${patronHex.take(8)}… as ${by.take(8)}… — the departure is ${hex(t).take(8)}…"
        }
    }

    /** Every identity this device holds, in the order the index names. */
    fun identities(context: Context): List<Identities.Held> =
        Identities.decode(AndroidShell(context.applicationContext).unseal(Identities.INDEX))

    /** Which identity acts, or null where the device holds none. */
    fun activeIdentity(): String? = activeId

    /**
     * **Choose the identity that acts** — a meeting's and an operator
     * action's ([Identities.needsTheActive]), from the nameplate's own
     * list [author, 2026-11-07]. Nothing where `id` is not held: a
     * selector offering what the device does not hold is a bug, and
     * switching to it would leave the device unable to meet.
     */
    fun chooseActive(context: Context, id: String): Boolean {
        if (!loaded.containsKey(id)) return false
        activeId = id
        AndroidShell(context.applicationContext).seal(Identities.ACTIVE, id.toByteArray())
        Diag.event("identity.active", "who" to id.take(8))
        participant?.let { show(it, Connection.Detached) }
        Model.refreshAll()
        return true
    }

    /**
     * **Open a backup into this client** (`rhtn_client::backup`;
     * `light-client-requirements.md` §2): the blob opens under this
     * identity, the import scans it, and what came back is reported —
     * records taken, and captures the scan discarded as past the window
     * their *subject* declared (design §13.7.1).
     *
     * Refused where this client already holds an archive, where the blob
     * belongs to another identity, or where it does not open; the kernel's
     * own words come back rather than a guess at them.
     *
     * **Waits for the kernel**, as the first run calls this moments after
     * starting it.
     */
    fun restoreBackup(blob: ByteArray): Result<uniffi.rhtn_ffi.Restored> {
        val until = System.currentTimeMillis() + 30_000
        var p = participant
        while (p == null && System.currentTimeMillis() < until) {
            Thread.sleep(200)
            p = participant
        }
        val held = p ?: return Result.failure(IllegalStateException("the kernel did not start"))
        return runCatching { held.restoreBackup(blob) }
    }

    /**
     * **The seeds this device holds**, for a rotation to sign its
     * `Recovery` with (design §9.0: *"the old key is in hand wherever the
     * rotation carries its inheritance, since it signs the `Recovery`"*).
     *
     * **The one caller is [beginRotation]'s screen**, and the copy is the
     * caller's to wipe. Nothing else has business reading these: a seed
     * crosses no wire (design §23.3), and this returns it only so the
     * device can sign with the key it already has rather than asking a
     * person to type a phrase it holds.
     */
    fun heldSeedsForRotation(context: Context): ByteArray? {
        // **the identity being rotated is the one that acts** ([Identities]:
        // a rotation is a meeting, and a meeting takes the active
        // identity), so its seeds come from its own scope and not the
        // top-level directory a single-identity device used
        val app = context.applicationContext
        val id = activeId
        val scope = if (id == null) {
            Identities.LEGACY_SCOPE
        } else {
            val held = identities(app).firstOrNull { it.id == id } ?: return null
            held.scope
        }
        return AndroidShell(app, scope).unseal(Identities.LEGACY_SEEDS)?.takeIf { it.size == 64 }
    }

    /**
     * **The departure a concluded rotation owes** [author, 2026-11-07]:
     * *"the rotation completes, the new key is adopted by patron P, and
     * then the prior key departs P"* — in that order, so the person is
     * never briefly outside the subnet they are rotating inside (design
     * §9.1).
     *
     * **The mark is the trigger and forgetting it is the completion**, the
     * same shape the rest of the rotation uses: this runs at every start
     * and at the end of every meeting, does nothing where there is no
     * mark, and does nothing where the new identity holds no line yet —
     * which is the state between beginning a rotation and meeting the
     * patron. A process that dies in the middle leaves the mark, and the
     * next start owes the same act.
     *
     * **Nothing is guessed about which subnet.** The patron is the one
     * the new identity's own line names; a new identity's archive is new
     * ([beginRotation]), so the line it holds is the adoption the
     * rotation just made.
     */
    private fun concludeRotation() {
        val app = appContext ?: return
        for (h in identities(app)) {
            val own = AndroidShell(app, h.scope)
            val priorId = own.unseal(Rotation.MARK_SUCCEEDS)
                ?.toString(Charsets.UTF_8)?.trim()
                ?.takeIf { Identities.names(it) }
                ?: continue
            val fresh = loaded[h.id] ?: continue
            // the rotation has not reached its adoption yet: nothing owed
            val patron = call("lines", { null }) { fresh.lines().firstOrNull()?.patron }
                ?: continue
            val old = loaded[priorId] ?: run {
                // the key being succeeded is not open here — taken off the
                // device, or never held. Nothing can sign its departure,
                // and the mark is spent rather than retried forever
                own.forget(Rotation.MARK_SUCCEEDS)
                front.note("· the key this rotation succeeded is not on this device, so its departure was not signed")
                return@run null
            } ?: continue
            val said = call("depart", { e -> e.reason }) {
                old.depart(patron, null)
                null
            }
            if (said == null) {
                own.forget(Rotation.MARK_SUCCEEDS)
                Diag.event("rotation.departed", "who" to priorId.take(8))
                front.note("· the key you rotated away from has left that subnet")
            } else {
                // **left for the next start**: the mark stays, so the
                // departure is owed rather than lost
                front.note("· that departure was refused and is still owed: $said")
            }
        }
    }

    /**
     * **Begin a key rotation, in a way a dying process cannot spoil**
     * (design §9.0, §9.1; PRD-03).
     *
     * The two seed pairs are sealed as a mark *before* anything is opened,
     * and [bringUp] obeys a mark over the sealed seeds — so the rotation
     * either has not begun or will be finished by some later start, and
     * there is no state in which the archive on disk belongs to a key
     * nothing names. The mark is cleared only after the new seeds are the
     * sealed ones.
     *
     * **Both halves or neither**: `wire-format.md` §4.1 admits no variant
     * of a `Recovery` carrying one, so a mark without its prior key is
     * dropped rather than half-performed.
     *
     * False where custody refused either, and nothing was begun. The
     * caller's copies are this function's to wipe.
     */
    fun beginRotation(context: Context, newSeeds: ByteArray, priorSeeds: ByteArray): Boolean {
        require(newSeeds.size == 64) { "the kernel's seeds are 64 bytes" }
        require(priorSeeds.size == 64) { "a prior identity is 64 bytes of seed" }
        val app = context.applicationContext
        // **the new key is a new identity, with an archive of its own**
        // [author, 2026-11-07]: *"just start a new archive for the new
        // identity … The old archive should show the old ID leaving the
        // subnet that rotated, but should otherwise remain unchanged."*
        // So the marks are sealed in the **new** identity's scope, not the
        // rotating one's: [openOn] then performs the rotation on empty
        // storage, and the key being succeeded opens from its own
        // directory untouched.
        val newId = idOfSeeds(newSeeds) ?: return false
        val priorId = idOfSeeds(priorSeeds) ?: return false
        if (newId == priorId) return false
        val scope = Identities.scopeFor(newId)
        val own = AndroidShell(app, scope)
        // the prior key first: a mark whose new seeds landed and whose
        // prior did not is the half-mark [openOn] has to drop, so make it
        // the harder half to be missing
        if (!own.seal(Rotation.MARK_PRIOR, priorSeeds)) return false
        if (!own.seal(Rotation.MARK_NEW, newSeeds)) {
            own.forget(Rotation.MARK_PRIOR)
            return false
        }
        // **which key it succeeds, kept for the departure that follows**
        // [author, 2026-11-07]: the old key leaves the rotated subnet once
        // the new one is adopted into its place, and the marks that hold
        // the prior seeds are forgotten the moment the rotation completes
        own.seal(Rotation.MARK_SUCCEEDS, priorId.toByteArray())
        // **and the index names it**, so a start opens it beside the one
        // it succeeds rather than instead of it
        val device = AndroidShell(app)
        val index = Identities.decode(device.unseal(Identities.INDEX))
        if (index.none { it.id == newId } &&
            !device.seal(Identities.INDEX, Identities.encode(index + Identities.Held(newId, scope, null)))
        ) {
            own.forget(Rotation.MARK_NEW)
            own.forget(Rotation.MARK_PRIOR)
            own.forget(Rotation.MARK_SUCCEEDS)
            return false
        }
        newSeeds.fill(0)
        priorSeeds.fill(0)
        Diag.event("rotation.begun")
        if (started) {
            openHeld(app, Identities.Held(newId, scope, null), listOf())
            chooseActive(context, newId)
        } else {
            start(context)
        }
        return true
    }

    /** The default name the kernel holds, or null. */
    fun defaultAlias(): String? {
        val p = participant ?: return null
        return call("defaultAlias", { null }) { p.defaultAlias() }
    }

    /**
     * Set the default name [author, 2026-10-27]: the kernel keeps it and
     * publishes it to each subnet the person has not named themselves
     * differently in. The refusal's words, or null where it was kept.
     * **Waits for the kernel**: the first run sets this moments after
     * starting it.
     */
    fun setDefaultAlias(name: String): String? {
        val until = System.currentTimeMillis() + 30_000
        var p = participant
        while (p == null && System.currentTimeMillis() < until) {
            Thread.sleep(200)
            p = participant
        }
        if (p == null) return "the kernel did not come up"
        return call("setDefaultAlias", { it.reason }) { p.setDefaultAlias(name); null }
    }

    /**
     * **Name a subnet this key is the root of** (`wire-format.md` §7.11
     * kind 1; `light-client-requirements.md` §4.4): the root's by
     * convention, which nothing enforces. Asked for where an adoption
     * forms one [author, 2026-10-29]. The refusal's words, or null.
     */
    fun publishSubnetAlias(subnetHex: String, alias: String): String? {
        val p = participant ?: return "the kernel is not up"
        return call("publishProfile", { it.reason }) { p.publishProfile(unhex(subnetHex), 1u, alias, null, null); null }
    }
}
