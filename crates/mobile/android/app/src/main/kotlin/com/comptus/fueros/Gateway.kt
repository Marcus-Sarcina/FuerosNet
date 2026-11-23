package com.comptus.fueros

import android.content.Context
import org.json.JSONObject
import uniffi.rhtn_ffi.Answer
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
 * kernel is Attachments.started here once, screens bind to its [front] and unbind from
 * it, and a screen that has gone is a null sink rather than a leaked loop.
 * Every context used is the application's, so nothing here retains an
 * Activity.
 */
object Gateway {

    /** The state a screen renders. Screens read it and bind to it; only the
     *  kernel mutates it. It lives in [Front], which a JVM test can hold. */
    val front = Front()

    internal val lock = Any()

    /**
     * **Every identity this device holds, open at once**, by its id
     * ([Identities]). A key rotation replaces the key one subnet knows and
     * leaves the others live (design §9.0), so a device that has rotated
     * holds a key still usable elsewhere [author, 2026-11-07].
     *
     * Each has storage of its own ([AndroidShell]'s scope): two kernels
     * over one directory would each overwrite the other's archive.
     */
    internal val loaded = java.util.concurrent.ConcurrentHashMap<String, Participant>()

    /**
     * **An identity's own serving node**, in its own scope — step 4 of the
     * browser model [author, 2026-11-11]: the node an identity agrees to
     * ([serveFrom]) is that identity's tab and not the device's, so an
     * identity attaches only to a node it agreed to ([launchHeld]). The
     * device-level provision stays the bench's convenience for the
     * identity a phone was set up with, read by [bringUp] for the one in
     * focus alone.
     */

    /** The application's context, kept for the radio the bearer needs. */
    @Volatile internal var appContext: Context? = null

    /** Bind a screen: it is rendered against the state so far, then on
     *  every change. */
    fun bind(u: Front.Ui) = front.bind(u)

    /** Unbind a screen: it is rendered no more. */
    fun unbind(u: Front.Ui) = front.unbind(u)

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
     * **An instance enrolled before this key roots a subnet is
     * unassigned** [author, 2026-11-21]: everything else happens, the
     * proof and the run, and the two records wait for the subnet its
     * operator chooses ([assign]).
     *
     * @param at `host:port` of the instance's surface
     * @param tokenHex the enrolment token its configuration carries
     * @param credentials how many 48-hour delegations the run carries
     * @param endpoint the address to name in an endpoint record, or null
     * @param asn the provider's autonomous system, where the caller knows it
     * @param zone that provider's own name for the place, beside the ASN
     */
    fun enrol(
        by: String,
        at: String,
        tokenHex: String,
        credentials: Int,
        endpoint: String?,
        /** The subnet the instance serves, hex, or null for the one under
         *  this device's own key: the records are signed on that line's
         *  series (`wire-format.md` §7.6). Null where this key roots no
         *  subnet yet signs neither, and the lines say [SERVES_NONE]. */
        subnet: String? = null,
        /** **Where the machine is, as the launch chose it** [author,
         *  2026-11-06]: the ASN and the provider's own zone label, which
         *  ride in both records below so a horizon can be ordered by the
         *  pair later (design §3.4). Null from the hand-typed enrolment,
         *  which was told neither. */
        asn: Long? = null,
        zone: String? = null,
    ): List<String> {
        val p = loaded[by] ?: return listOf("the kernel is not running")
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
        // no seed to sign its own; **or neither, for an instance that
        // serves no subnet yet** [author, 2026-11-21]: with no subnet named
        // and none rooted under this key, the line under it is the subnet
        // the next adoption creates, and the kernel refuses to place the
        // instance there unasked
        if (subnet == null && unassigned(by)) {
            said += SERVES_NONE
        } else {
            val placed = place(by, at, endpoint, subnet, asn, zone)
            said += placed.said
            if (!placed.signed) return said
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
     * **What an enrolment says of an instance that serves no subnet**,
     * and what a caller reads off its lines to record it so, as it reads
     * a taken run off the `credential` lines.
     */
    const val SERVES_NONE: String =
        "it serves no subnet until you choose one, so neither its endpoint record nor its " +
            "anchor entry is signed: the Operator screen offers it every subnet you come to hold, " +
            "and every adoption you make as patron asks whether this node serves that subnet"

    /**
     * **Whether this identity's key roots no subnet**, by the kernel's
     * own refusal (`Abort::Unassigned`) [author, 2026-11-21]: a record on
     * the line under its own key is refused until its first adoption as
     * patron creates one. Nothing is kept: the record asked for here is
     * over no points and is dropped, and any other answer, signed or
     * refused for another reason, is not this one.
     */
    fun unassigned(by: String): Boolean {
        val p = loaded[by] ?: return false
        return call("endpointRecord", { it.reason.contains("Unassigned") }) {
            p.endpointRecord(null, emptyList(), 1u)
            false
        }
    }

    /** What [place] came to: its lines; whether both records were signed;
     *  whether the instance took both. */
    data class Placed(val said: List<String>, val signed: Boolean, val taken: Boolean)

    /**
     * **Place an instance on a subnet's line** (`infra-client-requirements.md`
     * §4.4; `wire-format.md` §7.6): sign the endpoint record, where there
     * is an address to publish, and the anchor entry with nothing beneath
     * it, on the line `subnet` names (null: the one under this key), and
     * hand both to the instance at `at`. What an enrolment signs, and what
     * an assignment signs later for an instance enrolled unassigned.
     */
    internal fun place(
        by: String,
        at: String,
        endpoint: String?,
        subnet: String?,
        asn: Long?,
        zone: String?,
    ): Placed {
        val p = loaded[by] ?: return Placed(listOf("the kernel is not running"), false, false)
        val said = mutableListOf<String>()
        var taken = true
        if (endpoint != null) {
            val point = point(endpoint, asn, zone)
                ?: return Placed(said + "`$endpoint` is not host:port", false, false)
            val line = subnet?.takeIf { it.length == 64 }?.let { unhex(it) }
            val record = call("endpointRecord", { null }) { p.endpointRecord(line, listOf(point), 1u) }
                ?: return Placed(said + "the endpoint record was not signed", false, false)
            val put = Provision.put(at, "/node/endpoint-record", record)
            taken = put.isSuccess
            said += "endpoint record naming $endpoint: " + put.fold({ it }, { "${it.message}" })
        }
        // **the anchor entry is always signed at enrolment** [author,
        // 2026-10-25]: its size is the kernel's to state (nothing beneath a
        // fresh instance), not the operator's to type
        val point = point(endpoint ?: at, asn, zone)
            ?: return Placed(said + "`$at` is not host:port", false, false)
        val line2 = subnet?.takeIf { it.length == 64 }?.let { unhex(it) }
        val entry = call("anchorEntryFresh", { null }) {
            p.anchorEntryFresh(line2, listOf(point), 1u)
        } ?: return Placed(said + "the anchor entry was not signed", false, false)
        val put = Provision.put(at, "/node/anchor-entry", entry)
        said += "anchor entry, nothing beneath it yet: " + put.fold({ it }, { "${it.message}" })
        return Placed(said, true, taken && put.isSuccess)
    }

    /**
     * **Assign an unassigned instance to the subnet `subnet` names** [author,
     * 2026-11-21]: any subnet this key holds a position in, chosen on the
     * Operator screen or as the answer to the question an adoption as
     * patron asks, into a new subnet or one held. The proof is checked
     * first, as at enrolment, and then [place] signs both records on the
     * subnet's line, over what the enrolment was told of where the
     * instance is. No run is signed: it holds its own.
     */
    fun assign(
        by: String,
        at: String,
        tokenHex: String,
        subnet: String,
        endpoint: String?,
        asn: Long?,
        zone: String?,
    ): Placed {
        val token = runCatching { Provision.unhex(tokenHex.trim()) }.getOrNull()
            ?.takeIf { it.size == 32 } ?: return Placed(listOf("the token is 64 lower-case hex digits"), false, false)
        val got = Provision.fetch(at).getOrElse { return Placed(listOf("${it.message}"), false, false) }
        val checks = call("enrolmentProofChecks", { false }) {
            uniffi.rhtn_ffi.enrolmentProofChecks(token, got.nonce, got.transportKey, got.proof)
        }
        if (!checks) {
            return Placed(
                listOf("the proof did not check: whatever answered at $at does not hold this instance's token. Nothing was signed."),
                false,
                false,
            )
        }
        return place(by, at, endpoint, subnet, asn, zone)
    }

    /**
     * **Sign an instance's records for the address it answers at now**
     * (`infra-client-requirements.md` §4.4, §9.4; `wire-format.md` §7.6):
     * a node moved, or a provider changed its address. The endpoint record
     * and the anchor entry are re-signed over `points`, at the next
     * counter; no run is signed, the node holding its own.
     *
     * **The next counter is the anchor entry's plus one, for both**: the
     * two are signed together at enrolment at 1 and only the anchor entry
     * is re-signed since (a renewal, where its subtree moved), so its
     * counter is never below the endpoint record's, and one past it
     * advances both. An equal counter over a changed list is the conflict
     * §7.6 exists to prevent, and the node refuses it [bench, 2026-11-17].
     */
    fun readdress(by: String, at: String, tokenHex: String, points: List<ByteArray>, subnet: String?): List<String> {
        val p = loaded[by] ?: return listOf("the kernel is not running")
        val token = runCatching { Provision.unhex(tokenHex.trim()) }.getOrNull()
            ?.takeIf { it.size == 32 } ?: return listOf("the token is 64 lower-case hex digits")
        val got = Provision.fetch(at).getOrElse { return listOf("${it.message}") }
        val checks = call("enrolmentProofChecks", { false }) {
            uniffi.rhtn_ffi.enrolmentProofChecks(token, got.nonce, got.transportKey, got.proof)
        }
        if (!checks) {
            return listOf("the proof did not check: whatever answered at $at does not hold this instance's token. Nothing was signed.")
        }
        val line = subnet?.takeIf { it.length == 64 }?.let { unhex(it) }
        val next = ((got.anchorSeqno ?: 0) + 1).toUInt()
        val said = mutableListOf<String>()
        val record = call("endpointRecord", { null }) { p.endpointRecord(line, points, next) }
            ?: return said + "the endpoint record was not signed"
        said += "endpoint record at seqno $next: " +
            Provision.put(at, "/node/endpoint-record", record).fold({ it }, { "${it.message}" })
        val entry = call("anchorEntry", { null }) { p.anchorEntry(line, points, got.subtree.toULong(), next) }
            ?: return said + "the anchor entry was not signed"
        said += "anchor entry at seqno $next: " +
            Provision.put(at, "/node/anchor-entry", entry).fold({ it }, { "${it.message}" })
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
        by: String,
        at: String,
        tokenHex: String,
        credentials: Int,
        /** The subnet this instance's records were signed on, hex, or null
         *  for the line under this device's own key: a re-signed anchor
         *  entry must carry the same series as the standing one or it
         *  supersedes nothing (`wire-format.md` §2.3). */
        subnet: String? = null,
        /** **Serving no subnet** [author, 2026-11-21]: it holds no anchor
         *  entry and none is signed for it, renewal or not. */
        unassigned: Boolean = false,
    ): List<String> {
        val p = loaded[by] ?: return listOf("the kernel is not running")
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
        if (unassigned) {
            said += "it serves no subnet yet, so it holds no anchor entry to re-sign"
        } else if (claims == null || seqno == null) {
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
     * **Take whatever an intake code carries.** A `Card`
     * (`wire-format.md` §2.4) is somebody's key and locator together with
     * the anchor that locator names and where it answers — which is what a
     * code handed over at a meeting carries [author, 2026-10-13] — and a
     * bare `Introduction` (§2.3) or a node's `EndpointRecord` (§7.6) are
     * taken too. The kernel decides which by parsing, so a person scans a
     * code without being asked what kind it is.
     *
     * One line saying what was taken, or why not.
     */
    fun takeIntake(by: String, bytes: ByteArray): String {
        val p = loaded[by] ?: return "the kernel is not running"
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
    fun ownCard(by: String): ByteArray? {
        val p = loaded[by] ?: return null
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
    internal fun point(hostPort: String, asn: Long? = null, zone: String? = null): ByteArray? {
        val host = hostPort.substringBeforeLast(':', "")
        val port = hostPort.substringAfterLast(':', "").toUShortOrNull() ?: return null
        if (host.isEmpty()) return null
        return call("networkPoint", { null }) {
            uniffi.rhtn_ffi.networkPoint(host, port, asn?.toULong(), zone?.takeIf { it.isNotBlank() })
        }
    }

    /** Seconds since the epoch, which is what a delegation's window is in. */
    internal fun now(): ULong = (System.currentTimeMillis() / 1000).toULong()

    /**
     * One call into the kernel, bracketed for the diagnostics
     * (`Robot/field-test-diagnostics.md`, section 3.6): `shell.call`
     * before, `shell.return` after with the elapsed milliseconds and,
     * where the kernel refused, its reason. The refusal is then handled
     * as it was: `onRefused` is the catch block, and returns what the
     * body does. In the releasable flavour the events go nowhere and the
     * try is what it was.
     */
    internal inline fun <T> call(
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

    // ---- D7, the Ceremony.adoption() the meeting carried --------------------------

    // ---- A3, the Ceremony.peering() the meeting carried ---------------------------

    // ---- the bootstrap, which is the shell's own object ----------------

    /** The alias a contact published, where one has reached this device. */
    fun aliasOf(by: String, keyHex: String): String? {
        val p = loaded[by] ?: return null
        return aliases(p)[keyHex]
    }

    /**
     * **The name a party goes by to the identity that holds the row**
     * [author, 2026-11-12]: a thread or a contact of one identity is named
     * by that identity's kernel, not the focused one's, which may not know
     * the party at all.
     */
    fun aliasOfHeld(by: String, keyHex: String): String? =
        loaded[by]?.let { aliases(it)[keyHex] } ?: aliasOf(by, keyHex)

    /**
     * **Fetch what every located contact published** (`wire-format.md`
     * §7.11): from wherever each locator leads, kept beside the contact
     * and shown by [People]. Whatever does not answer leaves the keyhash
     * standing.
     */
    fun requestProfiles(by: String) {
        val p = loaded[by] ?: return
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
        /** The page's own `content-security-policy`, handed to the engine. */
        val policy: String? = null,
        /** **Whether the page declares that it runs script**: its policy
         *  names `script-src` (`light-client-requirements.md` §8). */
        val runsScript: Boolean = false,
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

    internal fun answered(a: uniffi.rhtn_ffi.ResourceAnswer): Answered = Answered(
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
                policy = q.policy,
                runsScript = q.runsScript,
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
    fun answerUtility(by: String, id: ULong, outcome: uniffi.rhtn_ffi.UtilityOutcome): Answered? {
        val p = loaded[by] ?: return null
        return call("answerUtility", { null }) { answered(p.answerUtility(id, outcome)) }
    }

    /**
     * **The `authorized_keys` line a launch installs on a machine this
     * identity rents** (`light-client-requirements.md` §10): derived from
     * the signing seed, so a device restored from the phrase derives the
     * same one and the operator's shell comes back with their identity.
     */
    fun hostControlPublicKey(by: String): String? {
        val p = loaded[by] ?: return null
        return call("hostControlPublicKey", { null }) { p.hostControlPublicKey() }
    }

    /**
     * **This client's own runner, signed by the operator's host key**
     * (`infra-client-requirements.md` §9.4): what the runner-refresh act
     * carries to a machine through its instance, which cannot forge it.
     */
    fun hostRunnerRefresh(by: String): uniffi.rhtn_ffi.RunnerRefresh? {
        val p = loaded[by] ?: return null
        return call("hostRunnerRefresh", { null }) { p.hostRunnerRefresh() }
    }

    /**
     * **The private half, in the form `ssh -i` reads.** A secret: whoever
     * holds it has a shell on the machines this identity launched. Shown
     * to the person, never written by this shell, and derived again
     * wherever the identity is.
     */
    fun hostControlPrivateKey(by: String): String? {
        val p = loaded[by] ?: return null
        return call("hostControlPrivateKey", { null }) { p.hostControlPrivateKey() }
    }

    /**
     * **The `authorized_keys` entry for the key this identity hands its
     * own instance**: bound by the host to the package runner and nothing
     * else, so installing a resource is an act through this client rather
     * than a shell on the machine.
     */
    fun hostRunnerAuthorizedLine(by: String): String? {
        val p = loaded[by] ?: return null
        return call("hostRunnerAuthorizedLine", { null }) { p.hostRunnerAuthorizedLine() }
    }

    /**
     * **The private half of the runner's key**, which goes to the
     * instance with a request and is kept there no longer than the act.
     * Never the operator's own key, which opens a shell.
     */
    fun hostRunnerPrivateKey(by: String): String? {
        val p = loaded[by] ?: return null
        return call("hostRunnerPrivateKey", { null }) { p.hostRunnerPrivateKey() }
    }

    /**
     * **Sweep the node serving a subnet, as the identity that holds it**
     * (`light-client-requirements.md` §8) [author, 2026-11-23: *"Sweep
     * when the user opens the subnet"*]: its catalog and its page, and the
     * icon of anything new on them. The refusal, where there is one, which
     * goes to the diagnostics and not to the person.
     */
    fun browse(by: String, node: String): String? {
        val p = loaded[by] ?: return "that identity is not open here"
        var why: String? = null
        call("browse", { why = it.reason }) { p.browse(unhex(node)) }
        return why
    }

    /**
     * **Ask a resource for something** (`wire-format.md` §11.1), through
     * the node that served its entry. The message is an HTTP/1.1 request
     * and this shell composes the simplest one there is; what the resource
     * answers is the application's, and nothing is kept of it.
     */
    fun ask(node: String, resource: String, path: String): Answered? {
        val me = Identities.activeId ?: return null
        return askAs(me, node, resource, path)
    }

    /**
     * **Reach a resource as the identity that holds its catalog entry**
     * [author, 2026-11-07]: a resource is reached inside a subnet, which
     * is acting within one — so the entry's holder is who asks, and not
     * whichever identity happens to act ([Identities.Owned]).
     */
    fun askAs(by: String, node: String, resource: String, path: String): Answered? =
        requestAs(by, node, resource, "GET", path, null, null)

    /**
     * **One request to a resource, composed here** (`wire-format.md`
     * §11.1): the method, the path and, where there is one, a body under
     * its type and length — what a page's own form or script asks this
     * shell to carry to the same resource, through the kernel and the
     * node that served it, and nowhere else (`light-client-requirements.md`
     * §8) [author, 2026-11-15].
     */
    fun requestAs(
        by: String,
        node: String,
        resource: String,
        method: String,
        path: String,
        type: String?,
        body: ByteArray?,
    ): Answered? {
        val p = asHolder(by) ?: return null
        val head = StringBuilder("$method $path HTTP/1.1\r\nhost: resource\r\n")
        if (body != null) {
            if (type != null) head.append("content-type: $type\r\n")
            head.append("content-length: ${body.size}\r\n")
        }
        head.append("\r\n")
        val request = head.toString().toByteArray() + (body ?: ByteArray(0))
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
    fun keyhash(by: String): String? {
        val p = loaded[by] ?: return null
        return call("me", { null }) { hex(p.me()) }
    }

    /**
     * **A fresh identity for a resource, of which only the keyhash is
     * kept** (`fresh_resource_keyhash`): an offering cannot carry one and
     * the operator should not be typing one.
     */
    fun mintResource(): String? =
        runCatching { uniffi.rhtn_ffi.freshResourceKeyhash() }.getOrNull()

    fun ownAlias(by: String): Pair<String?, String> {
        val p = loaded[by] ?: return null to ""
        val me = call("me", { "" }) { hex(p.me()) }
        // a subnet's own name for this person first, then the default the
        // foot carries [author, 2026-10-27]
        val named = subnets(by).firstNotNullOfOrNull { it.ownAlias?.takeIf { a -> a.isNotBlank() } }
            ?: call("defaultAlias", { null }) { p.defaultAlias() }
        return named to me
    }

    /** The subnet this device acts in, hex, where it is in one. */
    fun subnet(by: String): String? {
        val p = loaded[by] ?: return null
        return call("subnet", { null }) { p.subnet()?.let { hex(it) } }
    }

    // ---- the optical exchange, which is the kernel's ------------------

    /** The Attachments.peer this device talks to — adopted at a meeting or named by the
     *  provision — or null: what a conversation screen opens onto. */

    /**
     * **The bench's two levers on the Attachments.peer exchange** (fieldtest flavour,
     * by intent extra): forget one contact, so that a Attachments.peer's copy can be
     * watched bringing it back; and reach for the peers now rather than
     * at the clock's next tick. Each runs off the caller's thread and says
     * what it did on the home screen's notes.
     */
    fun forgetContact(by: String, hexKey: String) {
        val p = loaded[by] ?: run { front.note("· no kernel yet"); return }
        val who = runCatching { unhex(hexKey) }.getOrNull() ?: run { front.note("· not a key: $hexKey"); return }
        Thread({
            call("forget_contact", { e -> front.note("· forget refused: ${e.reason}") }) {
                val had = p.forgetContact(who)
                front.note(if (had) "· forgot contact ${hexKey.take(8)}…" else "· no such contact ${hexKey.take(8)}…")
            }
        }, "fueros-forget").start()
    }

    /** Every contact out, for the same watch; how many went is noted. */
    fun forgetContacts(by: String) {
        val p = loaded[by] ?: run { front.note("· no kernel yet"); return }
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
    fun providerCredentials(by: String): Map<String, String> {
        val p = loaded[by] ?: return emptyMap()
        val bytes = call("provider_credentials", { null }) { p.providerCredentials() } ?: return emptyMap()
        return runCatching {
            Json.obj(Json.parse(String(bytes))).mapNotNull { (k, v) -> Json.string(v)?.let { k to it } }.toMap()
        }.getOrDefault(emptyMap())
    }

    fun holdProviderCredential(by: String, provider: String, credential: String): String? {
        val p = loaded[by] ?: return "the kernel is not running"
        val all = providerCredentials(by).toMutableMap()
        if (credential.isBlank()) all.remove(provider) else all[provider] = credential
        val doc = "{" + all.entries.joinToString(",") { (k, v) -> "${Json.quote(k)}:${Json.quote(v)}" } + "}"
        return call("hold_provider_credentials", { e -> e.reason }) {
            p.holdProviderCredentials(if (all.isEmpty()) ByteArray(0) else doc.toByteArray())
            null
        }
    }

    /** How crowded a provider already is inside this operator's own
     *  horizon (PRD-11): the count, and whether it is marked. */
    fun crowded(by: String, asn: Long): Pair<Int, Boolean> {
        val p = loaded[by] ?: return 0 to false
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
    fun crowdedZones(by: String, asn: Long, zones: List<String>): Map<String, Pair<Int, Boolean>> {
        val p = loaded[by] ?: return emptyMap()
        if (zones.isEmpty()) return emptyMap()
        val rows = call("zones_by_concentration", { emptyList() }) {
            p.zonesByConcentration(asn.toULong(), zones)
        }
        return rows.associate { it.zone to (it.nodes.toInt() to it.crowded) }
    }

    /** This device's public key material, hex: what an instance is told
     *  to speak as. */
    fun materialHex(by: String): String? = loaded[by]?.material()?.joinToString("") { "%02x".format(it) }

    // ---- the device's own attach ---------------------------------------

    // ---- authority: the lines this key holds, and what ending one costs

    /**
     * **The aliases this device has been told**, by keyhash: what the
     * contacts list holds, which is where a published profile lands.
     * Absent for a party that has published none to this device, and
     * that absence is shown as the keyhash rather than filled in.
     */
    internal fun aliases(p: Participant): Map<String, String> =
        call("contacts", { emptyMap() }) {
            p.contacts().mapNotNull { c -> c.alias?.let { a -> c.keyhash.hex() to a } }.toMap()
        }

    /** The aliases held, by keyhash in hex: what a screen names a party by. */
    fun aliases(by: String): Map<String, String> = loaded[by]?.let { aliases(it) } ?: emptyMap()

    // ---- the nouns, as the model holds them --------------------------
    //
    // **Ports, not wrappers** (`Robot/implementation-plan.md` section 8):
    // each hands up the kernel's record as the model defines it — full
    // keyhashes, the wire's enums — and a screen's pure part decides what
    // to show of it. Nothing here shortens, words or renames.

    fun contacts(by: String): List<Contact> {
        val p = loaded[by] ?: return emptyList()
        return call("contacts", { emptyList() }) { p.contacts() }
    }

    /** **Who of this identity's contacts is in `subnet`** (hex), by name
     *  [author, 2026-11-20]: what an invitation's brief says. */
    fun contactsIn(by: String, subnet: String): List<String> {
        val p = loaded[by] ?: return emptyList()
        return call("contactsIn", { emptyList() }) { p.contactsIn(subnet) }
    }

    fun subnets(by: String): List<Subnet> {
        val p = loaded[by] ?: return emptyList()
        return call("subnets", { emptyList() }) { p.subnets() }
    }

    /** **An invitation read and its key placed** against what this device
     *  holds (`rhtn_client::invitation`); null where it is not one this
     *  build reads, and [invitationSay] gives the words for that. */
    fun invitation(by: String, text: String): uniffi.rhtn_ffi.Invited? {
        val p = loaded[by] ?: return null
        return call("invitation", { null }) { p.invitation(text) }
    }

    /** Why an invitation could not be read, in the words a person is told. */
    fun invitationSay(text: String): String =
        runCatching { uniffi.rhtn_ffi.invitationSay(text) }.getOrDefault("it could not be read")

    /** **Every row this device would assert about its horizon** to a node
     *  it operates (`rhtn_client::model::HorizonRow`): open bindings whose
     *  member's key material this device holds, each already in the form
     *  the administration surface reads. */
    fun horizonRows(by: String): List<uniffi.rhtn_ffi.HorizonRow> {
        val p = loaded[by] ?: return emptyList()
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
    fun declareNumber(by: String, number: String?) {
        val p = loaded[by] ?: return
        call("declareNumber", { }) { p.declareNumber(number) }
    }

    /**
     * **The node serving the subnet the meeting about to cross adopts
     * into, carried beside its intent** [author, 2026-11-21], where it
     * answers and whose it is [author, 2026-11-22], as
     * [OperatorStore.meetingNode] chose it, or null to clear it — which
     * every meeting that is not this patron's adoption into a subnet a
     * node serves does, so nothing reaches a later one. The point names no
     * provider and no zone: it rides an unsigned carriage
     * (`wire-format.md` §4.4).
     */
    fun declareNode(by: String, carried: OperatorStore.Carried?) {
        val p = loaded[by] ?: return
        val point = carried?.let { point(it.at) }
        val node = carried?.node?.takeIf { point != null }?.let { unhex(it) }
        call("declareNode", { }) { p.declareNode(point, node) }
    }

    /**
     * **The node this identity is served from in `subnet`**, hex, at the
     * first address it agreed for it, or null where nothing serves it
     * there: what it carries to a member it adopts there with no node of
     * its own [author, 2026-11-22].
     */
    fun servingIn(by: String, subnet: String): OperatorStore.Carried? {
        val p = loaded[by] ?: return null
        val a = call("servingIn", { null }) { p.servingIn(unhex(subnet)) } ?: return null
        val at = a.addresses.firstOrNull() ?: return null
        return OperatorStore.Carried(at, hex(a.serving))
    }

    /** The number held for a party, where their device reported one. */
    fun numberHeld(by: String, who: String): String? {
        val p = loaded[by] ?: return null
        if (!who.isHash32()) return null
        return call("numberHeld", { null }) { p.number(who.unhex()) }
    }

    /**
     * **The invitations this patron owes**, for a node answering at `at`:
     * where to send each and the link to send. Reading is also what drops
     * an address past its subject's window.
     */
    fun invitations(by: String, at: String, subnet: String? = null, asLabel: String? = null): List<uniffi.rhtn_ffi.Invite> {
        val p = loaded[by] ?: return emptyList()
        return call("invitations", { emptyList() }) { p.invitations(at, subnet?.let { unhex(it) }, asLabel) }
    }

    /** The subnets this device roots and could bring under a patron. */
    fun subnetsRooted(by: String): List<Subnet> {
        val p = loaded[by] ?: return emptyList()
        return call("subnetsRooted", { emptyList() }) { p.subnetsRooted() }
    }

    /** The patron lines this key holds, newest series and all. */
    fun lines(by: String): List<Line> {
        val p = loaded[by] ?: return emptyList()
        return call("lines", { emptyList() }) { p.lines() }
    }

    /** The meetings this device took part in, newest first, as outcomes. */
    fun meetings(by: String): List<Meeting> {
        val p = loaded[by] ?: return emptyList()
        return call("meetings", { emptyList() }) { p.meetings() }
    }

    /** The infra nodes whose endpoint records this device holds. */
    fun instances(by: String): List<Instance> {
        val p = loaded[by] ?: return emptyList()
        return call("instances", { emptyList() }) { p.instances() }
    }

    /** What the catalog holds, with each node's portion marked. */
    fun resources(by: String): List<Resource> {
        val p = loaded[by] ?: return emptyList()
        return call("resources", { emptyList() }) { p.resources() }
    }

    /**
     * **What leaving `patronHex` costs** (`light-client-requirements.md`
     * §6), or why it could not be said — the kernel's own reason, so a
     * patron this key holds no relationship with reads differently from
     * a kernel that has not come up.
     */
    fun costOfLeaving(by: String, patronHex: String): Pair<Cost?, String> {
        val p = loaded[by] ?: return null to "the kernel is not up yet"
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
    fun departFrom(by: String, patronHex: String): String {
        val p = loaded[by] ?: return "no kernel yet"
        val who = runCatching { unhex(patronHex) }.getOrNull() ?: return "not a key: $patronHex"
        return call("depart", { e -> "refused: ${e.reason}" }) {
            val t = p.depart(who, null)
            front.note("· left ${patronHex.take(8)}…; the departure is ${hex(t).take(8)}…")
            "left ${patronHex.take(8)}… — the departure is ${hex(t).take(8)}…"
        }
    }

    /** Start the participant, attach where Attachments.provisioned, and pump its events. */
    @Volatile internal var shell: AndroidShell? = null

    internal fun unhex(s: String): ByteArray =
        ByteArray(s.length / 2) { ((s[2 * it].digitToInt(16) shl 4) + s[2 * it + 1].digitToInt(16)).toByte() }

    internal fun hex(b: ByteArray): String = b.joinToString("") { "%02x".format(it) }

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
    internal fun <T> acrossHeld(what: String, of: (Participant) -> List<T>): List<Identities.Owned<T>> {
        val app = appContext ?: return listOf()
        return Identities.identities(app).flatMap { h ->
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
    internal fun asHolder(by: String): Participant? = loaded[by]

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

    /** The default name the kernel holds, or null. */
    fun defaultAlias(by: String): String? {
        val p = loaded[by] ?: return null
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
        var p = Identities.activeId?.let { loaded[it] }
        while (p == null && System.currentTimeMillis() < until) {
            Thread.sleep(200)
            p = Identities.activeId?.let { loaded[it] }
        }
        if (p == null) return "the kernel did not come up"
        return call("setDefaultAlias", { it.reason }) { p.setDefaultAlias(name); null }
    }

    /**
     * **Name a subnet this key is the root of** (`wire-format.md` §7.11
     * kind 1; `light-client-requirements.md` §4.4): the root's by
     * convention, which nothing enforces. Asked for where an Ceremony.adoption()
     * forms one [author, 2026-10-29]. The refusal's words, or null.
     */
    fun publishSubnetAlias(by: String, subnetHex: String, alias: String): String? {
        val p = loaded[by] ?: return "the kernel is not up"
        return call("publishProfile", { it.reason }) { p.publishProfile(unhex(subnetHex), 1u, alias, null, null); null }
    }
}
