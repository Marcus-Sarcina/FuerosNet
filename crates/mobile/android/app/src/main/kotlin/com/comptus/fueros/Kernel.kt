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
    internal val loaded = java.util.concurrent.ConcurrentHashMap<String, Participant>()

    /**
     * **Which identity acts** — a meeting's and an operator action's
     * ([Identities.needsTheActive]), chosen by press and hold on the
     * nameplate. Null only where the device holds none.
     */
    @Volatile internal var activeId: String? = null

    /**
     * **The identity in focus, named at a call** [restructure step 4,
     * 2026-11-14]: the person's choice of which identity acts, a UI
     * input and nothing the kernel reasons from. Every function that acts
     * for an identity takes it as its first argument; a screen that acts
     * for the focused one passes this, and a screen given its identity
     * passes that. Empty where the device holds none, which every
     * function answers as it answered a missing focus.
     */
    fun focused(): String = activeId ?: ""
    @Volatile internal var peer: ByteArray? = null
    @Volatile internal var peerKey: String? = null
    internal var peerName: String = "peer"
    /** Who this device nominates to witness, where the provision names
     *  anybody: a bench stand-in for the horizon a phone at genesis does
     *  not have (see [beginCeremony]). */
    @Volatile internal var nominees: List<ByteArray> = listOf()
    internal var started = false
    internal var provisioned = false

    /**
     * **An identity's own serving node**, in its own scope — step 4 of the
     * browser model [author, 2026-11-11]: the node an identity agrees to
     * ([serveFrom]) is that identity's tab and not the device's, so an
     * identity attaches only to a node it agreed to ([launchHeld]). The
     * device-level provision stays the bench's convenience for the
     * identity a phone was set up with, read by [bringUp] for the one in
     * focus alone.
     */

    /** An identity's conversations, in its own scope ([Threads]). */
    private const val THREADS = "threads"

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
        by: String,
        at: String,
        tokenHex: String,
        credentials: Int,
        /** The subnet this instance's records were signed on, hex, or null
         *  for the line under this device's own key: a re-signed anchor
         *  entry must carry the same series as the standing one or it
         *  supersedes nothing (`wire-format.md` §2.3). */
        subnet: String? = null,
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
            // a sweep names the node it reads: one per node this identity
            // holds a session with [restructure step 1, 2026-11-14]
            val held = call("attachments", { emptyList() }) { p.attachments() }
            if (held.isEmpty() && why == null) why = "no node is attached"
            for (node in held) {
                call("browse", { if (why == null) why = it.reason }) { p.browse(node) }
            }
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

    /** The peer this device talks to — adopted at a meeting or named by the
     *  provision — or null: what a conversation screen opens onto. */

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
    fun serveFrom(by: String, context: Context, nodeHex: String, address: String) {
        // **the node an identity agrees to is that identity's** — step 4
        // of the browser model [author, 2026-11-11]. Written into the
        // device's provision before this, so that with an attach per
        // identity every identity on the device would have joined a node
        // one of them agreed to at the next start: a profile joining a
        // server it never agreed to. Remembered in the focused identity's
        // own scope instead, which is the one that agreed (the kernel's agreement).
        val app = context.applicationContext
        val scope = activeId?.let { id -> identities(app).firstOrNull { it.id == id }?.scope }
        // **one tab per address** [author, 2026-11-11], **kept by the kernel**
        // [review, 2026-11-13: the one store of what an identity agreed to]
        val p = loaded[by] ?: run { front.note("· no kernel yet"); return }
        val node = Attach.node(nodeHex)?.let { runCatching { unhex(it) }.getOrNull() }
        if (node == null) {
            front.note("· not a node: ${nodeHex.take(8)}…")
            return
        }
        call("agreeTo", { e -> front.note("· the node could not be remembered: ${e.reason}") }) {
            p.agreeTo(node, listOf(address))
        }
        attachTo(by, nodeHex, address)
    }

    /** The bench's lever: `<node hex>@<host:port>` agreed to by the identity in focus. */
    fun agreeToNow(by: String, spec: String) {
        val p = loaded[by] ?: return
        val cut = spec.indexOf('@')
        if (cut <= 0) return
        val node = runCatching { unhex(spec.substring(0, cut)) }.getOrNull() ?: return
        val addr = spec.substring(cut + 1)
        call("agreeTo", { e -> Diag.warn("agree_to", "why" to e.reason) }) { p.agreeTo(node, listOf(addr)) }
        Diag.event("agree_to", "node" to spec.take(8), "addr" to addr)
    }

    /** The kernel's sealing service for one identity, as [Threads] reaches it. */
    private fun sealerOf(p: Participant) = object : Threads.Sealer {
        override fun seal(label: String, bytes: ByteArray): ByteArray = p.seal(label, bytes)
        override fun unseal(label: String, bytes: ByteArray): ByteArray? = runCatching { p.unseal(label, bytes) }.getOrNull()
    }

    /** The one JSON file of 2026-11-12, written out as sealed records once and forgotten. */
    private fun migrateThreads(shell: AndroidShell, s: Threads) {
        val old = shell.read(THREADS) ?: return
        val threads = Threads.decodeLegacy(old)
        for (t in threads) {
            for (m in t.messages) s.append(t.peerKey, Threads.Record.Said(m.mine, m.who, m.text, m.delivery, m.reason))
        }
        if (threads.isNotEmpty()) s.writeIndex(threads.map { Threads.Entry(it.peerKey, it.peerName, it.direct) })
        shell.forget(THREADS)
        Diag.event("threads.migrated", "threads" to threads.size, "messages" to threads.sumOf { it.messages.size })
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
     * bubble to *sent* — accepted for Ceremony.carriage, never *delivered* — or to
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
    fun send(by: String, to: String, text: String) {
        // **as the identity that holds the thread, never the one in
        // focus** — step 2b of the browser model [author, 2026-11-11]: a
        // reply typed into another identity's conversation went out under
        // the focused key before this, which is the wrong-key act the
        // row-holder rule exists to prevent, and nothing would have
        // refused it. Refused here rather than fallen back from
        // ([asHolder]).
        val p = asHolder(by)
        val who = runCatching { unhex(to) }.getOrNull()
        if (p == null || who == null || who.size != 32) {
            front.note(
                if (p == null) "· that identity is not open on this device" else "· not a party to send to: ${to.take(8)}…",
            )
            return
        }
        val id = front.outgoing(by, to, text)
        Thread({
            call("send", { e -> front.settle(by, to, id, Front.Delivery.UNSENT, e.reason) }) {
                p.send(who, kindApplication(), text.toByteArray())
                front.settle(by, to, id, Front.Delivery.SENT, null)
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
     * reconnect case, which a clock alone would Ceremony.meet() late. Off the
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
                            val round = loaded.values.flatMap { q -> q.networkChanged() }
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
    fun attachTo(by: String, nodeHex: String, address: String) {
        val p = loaded[by] ?: run { front.note("· no kernel yet"); return }
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
    fun detachNow(by: String) {
        val p = loaded[by] ?: run { front.note("· no kernel yet"); return }
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
    fun invitations(by: String, at: String): List<uniffi.rhtn_ffi.Invite> {
        val p = loaded[by] ?: return emptyList()
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
    fun exportBackup(by: String): ByteArray? {
        val p = loaded[by] ?: return null
        return call("exportBackup", { null }) { p.exportBackup() }
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

    fun forgetCaptures(by: String, hexKey: String) {
        val p = loaded[by] ?: run { front.note("· no kernel yet"); return }
        val who = runCatching { unhex(hexKey) }.getOrNull() ?: run { front.note("· not a key: $hexKey"); return }
        Thread({
            call("forget_captures", { e -> front.note("· forget refused: ${e.reason}") }) {
                val n = p.forgetCaptures(who)
                front.note("· forgot $n sealed capture(s) of ${hexKey.take(8)}…")
            }
        }, "fueros-forget").start()
    }

    fun contactPeersNow(by: String) {
        val p = loaded[by] ?: run { front.note("· no kernel yet"); return }
        Thread({
            call("contact_peers", { e -> front.note("· contact refused: ${e.reason}") }) {
                val round = p.contactPeers()
                Diag.event("peer.round", "peers" to round.size, "held" to round.count { it.held }, "how" to round.joinToString(",") { it.how })
                front.note("· peers: " + round.joinToString("; ") { "${hex(it.who).take(8)}… ${it.how}" }.ifEmpty { "none" })
            }
        }, "fueros-contact").start()
    }

    /** Start the participant, attach where provisioned, and pump its events. */
    @Volatile internal var shell: AndroidShell? = null

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
        // **a bench lever's choice is read here and not at the door**
        // [bench, 2026-11-11]: `chooseActive` refuses an identity that is
        // not open yet, and a lever read from the launching intent runs
        // before this loop has opened any — so the request joins the saved
        // choice, which [Identities.active] already falls back from where
        // the identity named is not held.
        activeId = Identities.active(index.filter { loaded.containsKey(it.id) }, requestedActive ?: saved)
        requestedActive = null
        // **a departure a rotation owes is owed at every start**, since a
        // process that died between the Ceremony.adoption() and the departure left
        // the mark behind ([concludeRotation])
        concludeRotation()
        val p = activeId?.let { loaded[it] } ?: run {
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
        activeId?.let { by -> peerKey?.let { front.peerKnown(by, it, peerName) } }
        // **a provision may name no node, and then this device stays
        // detached** (design §6.4): a pair can Ceremony.meet() with nothing reachable
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
        // **the tabs the focused identity agreed to** [author, 2026-11-11]:
        // each attached beside the provision's node, a second node while
        // one is held being a tab rather than a replacement
        run {
            val scope = activeId?.let { id -> index.firstOrNull { it.id == id }?.scope }
            var missed = 0
            for (t in call("agreed", { emptyList() }) { p.agreed() }) {
                val hex = t.serving.hex()
                if (hex == nodeHex) continue
                val a = call("attach", { e -> front.note("· tab ${hex.take(8)}… refused: ${e.reason}"); null }) {
                    p.attach(t.serving, t.addresses, listOfNotNull(to))
                }
                if (a == null) missed += 1
                Diag.event("identity.attach", "who" to (activeId ?: "").take(8), "node" to hex.take(8), "attached" to (a != null), "tab" to true)
            }
            // **and tried again where one did not answer** [bench, 2026-11-13:
            // the focused identity's attach timed out at start and nothing
            // retried it — the held identities' path did]
            if (missed > 0) activeId?.let { reattachLater(it, p, scope, 1) }
        }
        // **every other identity attaches and receives for itself** — step
        // 3 of the browser model [author, 2026-11-11]: the one in focus
        // stays on this thread, as it always did, and each of the rest
        // gets its own ([launchHeld])
        // **every identity's conversations, back from its own scope, and
        // written there after each change** [author, 2026-11-12]
        // **sealed through the kernel, appended per message, read when
        // opened** [author, 2026-11-14] (`light-client-requirements.md`
        // §9, APP-015): one store per identity over its own scope and its
        // own kernel's sealing; the 12th's one file migrated once
        appContext?.let { a ->
            val stores = HashMap<String, Threads>()
            fun storeOf(by: String): Threads? {
                stores[by]?.let { return it }
                val h = index.firstOrNull { it.id == by } ?: return null
                val p = loaded[by] ?: return null
                val shell = AndroidShell(a, h.scope)
                return Threads(shell.files(), sealerOf(p), by).also { stores[by] = it }
            }
            for (h in index) {
                val s = storeOf(h.id) ?: continue
                migrateThreads(AndroidShell(a, h.scope), s)
                front.restore(h.id, s.index())
            }
            front.appender = { by, peer, rec -> storeOf(by)?.append(peer, rec) }
            front.indexer = { by -> storeOf(by)?.writeIndex(front.entries(by)) }
            front.loader = { by, peer, before, pending -> storeOf(by)?.page(peer, before, Threads.PAGE, pending) }
        }
        for ((hid, hp) in loaded) {
            if (hid != activeId) launchHeld(hid, hp, index.firstOrNull { it.id == hid }?.scope)
        }
        runLoop(p, activeId ?: "")
    }

    /**
     * **An identity that is not in focus still receives, and still
     * attaches to a node it agreed to** — steps 3 and 4 of the browser
     * model [author, 2026-11-11]: a browser window out of focus keeps its
     * tabs open and its notifications arriving, and opens no tab it was
     * never told to. The node is the identity's own agreement, kept by the kernel,
     * written where it agreed ([serveFrom]); an identity with none —
     * one just added, or a rotation's new key before its patron — has no
     * node until it agrees to one, and is recorded detached rather than
     * handed the device's. The result is this identity's alone
     * ([Model.connectionOf]), and its loop runs on a thread of its own.
     *
     * **The node admits an identity or refuses it on its own terms**,
     * which is the author's ruling that a node serves an identity and not
     * a handset: a refusal is that identity detached, said to the
     * diagnostics and the model, and nothing else on the device changes.
     * Called by [bringUp] for every loaded identity but the focused one,
     * and by [createIdentity] and [beginRotation] for one added while the
     * kernel is up.
     */
    private fun launchHeld(id: String, p: Participant, scope: String?) {
        // **every tab the identity agreed to** [author, 2026-11-11]: the
        // first attach is the session, each further one a tab on it, and
        // the connection shown is the first's
        attachTabs(id, p, scope, attempt = 0)
        Thread({ runLoop(p, id) }, "fueros-kernel-${id.take(8)}").start()
    }

    /**
     * **Every tab the identity agreed to, attached** — the first attach the
     * session, each further one a tab on it; the connection shown is the
     * first's. Where one did not answer, [reattachLater] tries again.
     */
    private fun attachTabs(id: String, p: Participant, scope: String?, attempt: Int) {
        val tabs = call("agreed", { emptyList() }) { p.agreed() }
        if (tabs.isEmpty()) {
            Diag.event("identity.attach", "who" to id.take(8), "node" to "none")
            Model.connectionOf(id, Connection.Detached)
            return
        }
        var first: Connection? = null
        var missed = 0
        for (t in tabs) {
            val node = t.serving
            val hex = node.hex()
            val a = call("attach", { e -> Diag.event("identity.attach", "who" to id.take(8), "node" to hex.take(8), "refused" to e.reason); null }) {
                p.attach(node, t.addresses, listOf())
            }
            Diag.event("identity.attach", "who" to id.take(8), "node" to hex.take(8), "attached" to (a != null), "attempt" to attempt)
            if (a == null) missed += 1
            if (first == null) first = if (a == null) Connection.Detached else Connection.Attached(node, a.primary)
        }
        Model.connectionOf(id, first ?: Connection.Detached)
        if (missed > 0) reattachLater(id, p, scope, attempt + 1)
    }

    /** The waits before each attempt to attach again, in seconds; the last repeats. */
    private val REATTACH_AFTER_S = longArrayOf(15, 30, 60, 120, 300)

    private val reattaching = java.util.concurrent.ConcurrentHashMap<String, Int>()

    /**
     * **An attach that got no answer at start is tried again, with
     * backoff** [author, 2026-11-04]. A session *lost* after it held is
     * the kernel's to try again, beside its judgement of the loss
     * (`Net::watch`); this chain is for the attach that never held, which
     * the kernel has no session to watch. One chain per identity; a chain
     * already running is not doubled.
     */
    private fun reattachLater(id: String, p: Participant, scope: String?, attempt: Int) {
        if (reattaching.putIfAbsent(id, attempt) != null) return
        val wait = REATTACH_AFTER_S[minOf(attempt - 1, REATTACH_AFTER_S.size - 1).coerceAtLeast(0)]
        Diag.event("identity.reattach", "who" to id.take(8), "attempt" to attempt, "after_s" to wait)
        Thread({
            try {
                Thread.sleep(wait * 1000)
            } catch (_: InterruptedException) {
                reattaching.remove(id)
                return@Thread
            }
            reattaching.remove(id)
            attachTabs(id, p, scope, attempt)
        }, "fueros-reattach-${id.take(8)}").start()
    }

    /**
     * **The event loop, over one participant** — step 1 of the browser
     * model [author, 2026-11-11: *"Begin at 1"*]: extracted from [bringUp]
     * so that a loop can be run per identity, which is what lets an
     * identity receive while it is not the one in focus. **This step
     * changes no behaviour**: [bringUp] still runs exactly one, for the
     * identity that acts, on the thread it always did.
     *
     * What the loop still shares with the device rather than the identity
     * — the meeting ([Ceremony.meet()]), the front's notices, the model's refresh —
     * is step 2's to separate, and is left as it was here so that this
     * extraction can be read as a move and nothing else.
     */
    private fun runLoop(p: Participant, id: String) {
        // **the loop's own first word.** Said once, before any turn, so
        // that a run which never reaches the loop is distinguishable from
        // one whose loop parks in its first call (2026-10-24).
        Diag.event("kernel.loop", "state" to "entered", "who" to id.take(8))
        val diag = Ceremony.TurnDiag()
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
            Ceremony.turnEntry(diag, turns)
            turns += 1
            val e = p.nextEvent(2000UL)
            // **the meeting belongs to the identity in focus** — step 2 of
            // the browser model [author, 2026-11-11]: a meeting, the
            // Ceremony.adoption() and Ceremony.peering() it carries, and the tick that drives
            // them are the device's one at a time and the focused
            // identity's, where everything else below — a message, a
            // peer's word, a changed noun — is this identity's whichever
            // is in focus. Read every turn, since focus can move while
            // this loop runs. **No behaviour changes while bring-up runs
            // one loop for the identity that acts**: that loop is always
            // the focused one, and the unfocused branches are reached only
            // once step 3 runs a loop per identity.
            val focused = id == activeId
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
                        is Event.Conversed -> Ceremony.refusalIn(e.step)
                        else -> null
                    },
                )
            }
            if (e != null && Ceremony.took(id, p, e)) {
                // a meeting's own, taken by it
            } else {
                when (e) {
                    is Event.Payload -> {
                        val from = hex(e.from)
                        val who = if (from == peerKey) peerName else from.take(16)
                        front.incoming(id, from, who, String(e.bytes))
                    }
                    is Event.Connection -> {
                        // every identity's connection is recorded for itself
                        Model.connectionOf(id, e.v1)
                        if (focused) show(p, e.v1)
                    }
                    // **the nouns the kernel changed**: the shell's copy is
                    // refreshed for exactly those, and the screens watching
                    // them redraw
                    is Event.Changed -> Model.refresh(e.nouns.toSet())
                    // between peers (design §6.3): what a locator, a deposit, a
                    // request or a return came to, in the kernel's words
                    is Event.Peer -> {
                        Diag.event("peer.event", "from" to Diag.id8(e.from), "what" to e.what)
                        front.note("· peer: ${e.what}")
                    }
                    is Event.Answered -> front.note("· a response was refused: ${e.refused}")
                    is Event.Conversed -> front.note("· ${e.what}")
                    null -> {}
                    else -> front.note("· $e")
                }
            }
            Ceremony.tick(id, p, diag)
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
    internal fun show(p: Participant, s: Connection) {
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
                if (who.size == 32) loaded[t.by]?.let { h -> front.directPath(t.by, t.peerKey, h.directTo(who)) }
            }
        }
    }

    /** `dir` and everything under it gone, or false where some of it stays. */
    private fun clear(dir: java.io.File): Boolean {
        if (!dir.exists()) return true
        dir.listFiles()?.forEach { if (it.isDirectory) clear(it) else it.delete() }
        return dir.deleteRecursively() || !dir.exists()
    }

    internal fun unhex(s: String): ByteArray =
        ByteArray(s.length / 2) { ((s[2 * it].digitToInt(16) shl 4) + s[2 * it + 1].digitToInt(16)).toByte() }

    internal fun hex(b: ByteArray): String = b.joinToString("") { "%02x".format(it) }

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
        Ceremony.meet = null
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
            loaded[id]?.let { launchHeld(id, it, scope) }
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
     * **Every identity held, with its colour** [author, 2026-11-12]: the
     * device's colours file, each identity without one given the first
     * unused and the file written back where that happened.
     */
    fun identityColours(context: Context): Map<String, Int> {
        val shell = AndroidShell(context.applicationContext)
        val stored = Identities.colours(runCatching { shell.read(Identities.COLOURS) }.getOrNull())
        val now = Identities.assigned(stored, identities(context))
        if (now != stored) runCatching { shell.write(Identities.COLOURS, Identities.encodeColours(now)) }
        return now
    }

    /** The colour chosen for one identity on Options. */
    fun setIdentityColour(context: Context, id: String, argb: Int) {
        val shell = AndroidShell(context.applicationContext)
        val now = LinkedHashMap(identityColours(context))
        now[id] = argb
        runCatching { shell.write(Identities.COLOURS, Identities.encodeColours(now)) }
            .onFailure { front.note("· the colour could not be kept: ${it.message}") }
    }

    /**
     * **Choose the identity that acts** — a meeting's and an operator
     * action's ([Identities.needsTheActive]), from the nameplate's own
     * list [author, 2026-11-07]. Nothing where `id` is not held: a
     * selector offering what the device does not hold is a bug, and
     * switching to it would leave the device unable to Ceremony.meet().
     */
    /**
     * **Take one identity off this device, leaving the others** [author,
     * 2026-11-10]. The refusals are [Identities.removable]'s and are
     * returned as they are; null means it went.
     *
     * **The index is rewritten before the storage is deleted**, which is
     * the opposite order from [createIdentity] and for the same reason: a
     * process that dies between leaves storage nothing opens rather than
     * an index naming storage that is not there. Half-removed is the
     * failure to design against, and an orphaned directory is the
     * harmless half.
     *
     * **A rotation owed in that identity's scope stops it.** The marks say
     * a prior key is still to depart ([Rotation.MARK_PRIOR]); deleting the
     * identity that owes the departure would leave the old key standing in
     * a subnet it has been replaced in, with nothing on the device able to
     * say so.
     */
    fun removeIdentity(context: Context, id: String): String? {
        val app = context.applicationContext
        val device = AndroidShell(app)
        val index = Identities.decode(device.unseal(Identities.INDEX))
        Identities.removable(index, id)?.let { return it }
        val (kept, scope) = Identities.without(index, id)
        val own = AndroidShell(app, scope)
        if (own.unseal(Rotation.MARK_PRIOR) != null) {
            return "this identity owes a departure from a rotation; finish that first"
        }
        if (!device.seal(Identities.INDEX, Identities.encode(kept))) {
            return "this device's storage would not keep its list of identities"
        }
        // nothing opens it from here on: the index no longer names it
        loaded.remove(id)
        val left = scope?.let { java.io.File(app.filesDir, "${Wipe.KERNEL_DIR}/$it").deleteRecursively() } ?: false
        if (activeId == id) {
            Identities.active(kept.filter { loaded.containsKey(it.id) }, null)?.let { chooseActive(context, it) }
        }
        Diag.event("identity.removed", "who" to id.take(8), "storage" to left.toString())
        Model.refreshAll()
        return if (left) {
            null
        } else {
            "the identity is off this device's list; some of its storage would not go"
        }
    }

    /**
     * **Which identity the next bring-up should make active**, for the
     * bench's lever alone (`ShellActivity.benchLevers`): nothing in a
     * releasable build sets it, and it is spent by the bring-up that
     * reads it.
     */
    @Volatile
    private var requestedActive: String? = null

    /** Ask the next bring-up to make `id` the one that acts. */
    fun requestActive(id: String) {
        requestedActive = id.trim().takeIf { Identities.names(it) }
    }

    fun chooseActive(context: Context, id: String): Boolean {
        if (!loaded.containsKey(id)) return false
        activeId = id
        AndroidShell(context.applicationContext).seal(Identities.ACTIVE, id.toByteArray())
        Diag.event("identity.active", "who" to id.take(8))
        activeId?.let { loaded[it] }?.let { show(it, Connection.Detached) }
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
        var p = activeId?.let { loaded[it] }
        while (p == null && System.currentTimeMillis() < until) {
            Thread.sleep(200)
            p = activeId?.let { loaded[it] }
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
     * ([beginRotation]), so the line it holds is the Ceremony.adoption() the
     * rotation just made.
     */
    internal fun concludeRotation() {
        val app = appContext ?: return
        for (h in identities(app)) {
            val own = AndroidShell(app, h.scope)
            val priorId = own.unseal(Rotation.MARK_SUCCEEDS)
                ?.toString(Charsets.UTF_8)?.trim()
                ?.takeIf { Identities.names(it) }
                ?: continue
            val fresh = loaded[h.id] ?: continue
            // the rotation has not reached its Ceremony.adoption() yet: nothing owed
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
            loaded[newId]?.let { launchHeld(newId, it, scope) }
            chooseActive(context, newId)
        } else {
            start(context)
        }
        return true
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
        var p = activeId?.let { loaded[it] }
        while (p == null && System.currentTimeMillis() < until) {
            Thread.sleep(200)
            p = activeId?.let { loaded[it] }
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
