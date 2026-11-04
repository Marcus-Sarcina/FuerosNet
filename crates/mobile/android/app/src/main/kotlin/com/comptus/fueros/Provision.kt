package com.comptus.fueros

import java.io.ByteArrayOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.security.SecureRandom

/**
 * **The operator's side of an instance's administration surface**
 * (`infra-client-requirements.md` §8.2, §8.3).
 *
 * §8.3 has the client ship the provisioning pages, "because they must work
 * before any node exists": an instance comes up holding a transport key it
 * minted and nothing else, and it waits. What ends the wait is a run its
 * operator signed over that key — and the operator signs on a device that
 * holds the seed, which is this one.
 *
 * **The hazard this guards is substitution.** Something else answering at
 * that address would offer *its* key, and a run signed over it would
 * delegate this identity to a stranger. So the instance returns a proof
 * beside the key: `HMAC-SHA256` under the enrolment token, over a tag, the
 * nonce this device chose, and the key. The token was written into the
 * instance's configuration by its operator and is never sent. **Nothing is
 * signed until that proof checks**, and nothing below the check is
 * reversible: a credential handed over is one the far side keeps.
 *
 * **This object holds the exchange and no keys.** The signing is
 * [Kernel]'s, because the seed is; what is here is HTTP and parsing, which
 * is why a test can drive it against an ordinary socket.
 */
object Provision {

    /** How long one exchange may take, in milliseconds. */
    private const val PATIENCE = 10_000

    /** What an instance answered a fetch with. */
    data class Fetched(
        /** The transport public half it minted, 32 bytes. */
        val transportKey: ByteArray,
        /** The proof it offered over that key, as it sent it. */
        val proof: ByteArray,
        /** The nonce this device chose, which the proof is only good for. */
        val nonce: ByteArray,
        /** `enrolling` before a run arrives, `serving` after. */
        val phase: String,
        /** How many credentials its run already carries. */
        val credentials: Int,
        /** When its run ends, in seconds, or 0 where it holds none: what a
         *  renewal starts from (`infra-client-requirements.md` §7). */
        val runEnds: Long,
        /** `held`, `wanted` or `unconfigured`, for each of §4.4's records. */
        val endpointRecord: String,
        val anchorEntry: String,
        /** The subtree it counts beneath itself now (`§4.4`): what its
         *  anchor entry should claim, and the instance's fact to state
         *  rather than its operator's to know. */
        val subtree: Long,
        /** What its signed entry claims and at which counter, where one is
         *  signed: a claim that is not [subtree] owes a re-signing. */
        val anchorClaims: Long?,
        val anchorSeqno: Int?,
        /** The points that entry names, each as hex, so a re-signing moves
         *  the size and not the addresses. */
        val anchorPoints: List<String>,
    )

    /** Sixteen bytes of this device's own choosing, per fetch. */
    fun nonce(): ByteArray = ByteArray(16).also { SecureRandom().nextBytes(it) }

    /**
     * `GET /node?nonce=…`: the key and the proof over it.
     *
     * **The nonce is this caller's** and makes the answer unreplayable; a
     * proof returned under any other nonce is worthless, which is what
     * stops one captured earlier from standing in.
     */
    fun fetch(at: String, nonce: ByteArray = nonce()): Result<Fetched> = runCatching {
        val said = exchange(at, "GET", "/node?nonce=${hex(nonce)}", null).getOrThrow()
        Fetched(
            transportKey = unhex(field(said, "transport") ?: error("the answer carries no key")),
            proof = unhex(field(said, "proof") ?: error("the answer carries no proof")),
            nonce = nonce,
            phase = field(said, "phase") ?: "unknown",
            credentials = field(said, "credentials")?.toIntOrNull() ?: 0,
            runEnds = field(said, "run-ends")?.toLongOrNull() ?: 0,
            endpointRecord = field(said, "endpoint-record") ?: "unconfigured",
            anchorEntry = field(said, "anchor-entry") ?: "unconfigured",
            subtree = field(said, "subtree")?.toLongOrNull() ?: 0,
            anchorClaims = field(said, "anchor-claims")?.toLongOrNull(),
            anchorSeqno = field(said, "anchor-seqno")?.toIntOrNull(),
            anchorPoints = (field(said, "anchor-points") ?: "")
                .split(',')
                .filter { it.isNotEmpty() },
        )
    }

    /** `PUT` one signed object, and the first line of what came back. */
    fun put(at: String, path: String, body: ByteArray): Result<String> =
        exchange(at, "PUT", path, body).map { it.lineSequence().firstOrNull()?.trim() ?: "" }

    /**
     * **The target a management act carries**
     * (`infra-client-requirements.md` §8.2): the path, a nonce of this
     * device's own, and the proof over the act.
     *
     * **This is why the acts are drawn here and not in the node's page.**
     * A proof is `HMAC-SHA256` under the instance's token over the
     * method, the target without the proof, and the body — and HTML in a
     * frame can compute no such thing (§8.3). So the page declares and
     * this makes.
     *
     * The three routes that carry the operator's own signature need none
     * of this: a run and the two records verify themselves, so [put] is
     * unchanged.
     */
    fun proved(
        tokenHex: String,
        method: String,
        path: String,
        body: ByteArray,
    ): Result<String> = runCatching {
        val token = unhex(tokenHex.trim())
        require(token.size == 32) { "an administration token is 64 lower-case hex digits" }
        val n = nonce()
        val joiner = if (path.contains('?')) '&' else '?'
        // **the target as the instance will re-derive it**: everything a
        // caller could change is inside what the proof covers, and only
        // the proof itself is outside
        val signed = "$path${joiner}nonce=${hex(n)}"
        val proof = uniffi.rhtn_ffi.administrationActProof(token, n, method, signed, body)
        "$signed&proof=$proof"
    }

    /**
     * **One declared act, invoked** (`rhtn_daemon::acts`): the act's name
     * on the one route a client needs, with its declared fields as the
     * body, proved.
     *
     * The whole answer comes back rather than its first line: an act that
     * reached the machine has the host's own words in it, and an operator
     * reading them is being told something they can act on.
     */
    fun act(at: String, tokenHex: String, name: String, fields: List<Pair<String, String>>): Result<String> =
        runCatching {
            val body = form(fields)
            val target = proved(tokenHex, "POST", "/node/act/$name", body).getOrThrow()
            exchange(at, "POST", target, body, "application/x-www-form-urlencoded").getOrThrow()
        }

    /**
     * **One proved POST to a route that is not a declared act.**
     *
     * The declared acts are what an operator is *offered* on a page, with
     * their fields drawn as a form. The horizon routes are not that: a row
     * is hex a person has no reason to read or type, and putting them on
     * that surface would be plumbing in front of a user. So they are plain
     * routes, posted with the same proof.
     */
    fun post(
        at: String,
        tokenHex: String,
        path: String,
        fields: List<Pair<String, String>>,
    ): Result<String> = runCatching {
        val body = form(fields)
        val target = proved(tokenHex, "POST", path, body).getOrThrow()
        exchange(at, "POST", target, body, "application/x-www-form-urlencoded").getOrThrow()
    }

    /**
     * **Assert this client's whole horizon to a node it operates**, a row
     * at a time (`Robot/deferred-decisions.md`, the horizon as a table its
     * operator maintains).
     *
     * **A row at a time, not a table at once** [author, 2026-11-04:
     * "chunking is fine"]: one row is about two kilobytes, so nothing here
     * approaches `wire-format.md` §9.2's request-frame bound, and a row
     * that is refused does not take the others with it.
     *
     * **What the kernel decided is not re-decided here.** Which bindings
     * are rows, what each carries and what was left out for want of key
     * material are all settled by `Participant.horizonRows`; this posts
     * them and counts what the node said.
     */
    fun assertHorizon(
        at: String,
        tokenHex: String,
        rows: List<uniffi.rhtn_ffi.HorizonRow>,
    ): Result<Posted> = runCatching {
        var taken = 0
        val refused = mutableListOf<String>()
        for (r in rows) {
            val fields = buildList {
                add("node" to r.node)
                add("patron" to r.patron)
                add("series" to r.series.toString())
                add("revision" to r.revision.toString())
                add("material" to r.material)
                add("adoption" to r.adoption)
                add("from" to r.from.toString())
                r.anchor?.let { add("anchor" to it) }
                r.slot?.let { add("slot" to it.toString()) }
            }
            post(at, tokenHex, "/node/horizon", fields).fold(
                onSuccess = { taken += 1 },
                // **the member, and what the node said about it**: a row
                // refused for one member is not the others' problem, and an
                // operator reading this is being told which one
                onFailure = { refused += "${r.node.take(8)}: ${it.message ?: "refused"}" },
            )
        }
        Posted(taken, refused)
    }

    /** What asserting a horizon came to: rows the node took, and what it refused. */
    data class Posted(val taken: Int, val refused: List<String>)

    /** **What the instance declares it can be asked for**, as it sent it. */
    fun acts(at: String, tokenHex: String): Result<String> = runCatching {
        val target = proved(tokenHex, "GET", "/node/acts", ByteArray(0)).getOrThrow()
        exchange(at, "GET", target, null).getOrThrow()
    }

    /** `application/x-www-form-urlencoded`, as the surface reads it. */
    fun form(fields: List<Pair<String, String>>): ByteArray =
        fields.joinToString("&") { (k, v) -> "$k=${escape(v)}" }.toByteArray()

    /**
     * Percent-escaping, leaving the unreserved set alone.
     *
     * **Decided per byte and not per character**: a manifest is UTF-8 and
     * a label may be anything a person writes, so what is escaped is the
     * byte — a check on a character reconstructed from a signed byte is
     * wrong above 0x7f, which is where the interesting values are.
     */
    fun escape(s: String): String =
        s.toByteArray(Charsets.UTF_8).joinToString("") { b ->
            val v = b.toInt() and 0xff
            val unreserved = v in 0x41..0x5a || v in 0x61..0x7a || v in 0x30..0x39 ||
                v == '-'.code || v == '_'.code || v == '.'.code || v == '~'.code
            if (unreserved) v.toChar().toString() else "%%%02X".format(v)
        }

    /**
     * One exchange with the surface.
     *
     * **A status other than 200 is a refusal, not an answer**, and it
     * carries the surface's own words: an operator reading "the run is not
     * this operator's" is being told something they can act on, and
     * replacing it with a code would throw that away.
     */
    fun exchange(
        at: String,
        method: String,
        path: String,
        body: ByteArray?,
        contentType: String = "application/octet-stream",
    ): Result<String> =
        runCatching {
            val c = URL("http://$at$path").openConnection() as HttpURLConnection
            try {
                c.requestMethod = method
                c.connectTimeout = PATIENCE
                c.readTimeout = PATIENCE
                c.useCaches = false
                c.setRequestProperty("connection", "close")
                if (body != null) {
                    c.doOutput = true
                    c.setFixedLengthStreamingMode(body.size)
                    c.setRequestProperty("content-type", contentType)
                    c.outputStream.use { it.write(body) }
                }
                val code = c.responseCode
                val said = (if (code in 200..299) c.inputStream else c.errorStream)
                    ?.use { stream ->
                        val out = ByteArrayOutputStream()
                        stream.copyTo(out)
                        out.toString("UTF-8")
                    } ?: ""
                if (code !in 200..299) error("the surface refused: ${said.trim().ifEmpty { "$code" }}")
                said
            } finally {
                c.disconnect()
            }
        }

    /** One `name value` line of a fetch's answer. */
    fun field(said: String, name: String): String? =
        said.lineSequence()
            .firstOrNull { it.startsWith("$name ") }
            ?.removePrefix("$name ")
            ?.trim()

    fun hex(b: ByteArray): String = b.joinToString("") { "%02x".format(it) }

    /** Lower-case hex of an even length, or a refusal. */
    fun unhex(s: String): ByteArray {
        require(s.length % 2 == 0 && s.isNotEmpty()) { "`$s` is not hex" }
        require(s.all { it in "0123456789abcdef" }) { "`$s` is not lower-case hex" }
        return ByteArray(s.length / 2) { s.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
    }
}
