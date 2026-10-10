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
        /** `held`, `wanted` or `unconfigured`, for each of §4.4's records. */
        val endpointRecord: String,
        val anchorEntry: String,
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
            endpointRecord = field(said, "endpoint-record") ?: "unconfigured",
            anchorEntry = field(said, "anchor-entry") ?: "unconfigured",
        )
    }

    /** `PUT` one signed object, and the first line of what came back. */
    fun put(at: String, path: String, body: ByteArray): Result<String> =
        exchange(at, "PUT", path, body).map { it.lineSequence().firstOrNull()?.trim() ?: "" }

    /**
     * One exchange with the surface.
     *
     * **A status other than 200 is a refusal, not an answer**, and it
     * carries the surface's own words: an operator reading "the run is not
     * this operator's" is being told something they can act on, and
     * replacing it with a code would throw that away.
     */
    private fun exchange(at: String, method: String, path: String, body: ByteArray?): Result<String> =
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
                    c.setRequestProperty("content-type", "application/octet-stream")
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
