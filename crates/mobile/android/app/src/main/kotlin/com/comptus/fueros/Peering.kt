package com.comptus.fueros

/**
 * **The peering after the record** (A3; design §6.3, where a peering is a
 * transaction both peers sign and rests on a meeting; `wire-format.md`
 * §4.4, where a light client peers by locator).
 *
 * The same shape as [Adoption] and a simpler conversation, because a
 * peering has **no direction to settle**: it joins two parties who by
 * construction share no position, neither is over the other, and design
 * §6 prices its edge like a proof-of-presence edge. So four messages
 * rather than five — one side sends its back-pointers, the other builds
 * the body and shows it, the first reviews and signs or refuses, and the
 * builder assembles the record, takes it and sends the copy.
 *
 * **The lower keyhash builds the body** [2026-10-18]. Somebody has to,
 * since the body carries one timestamp and the two sides would otherwise
 * each build their own; nothing in the protocol makes either side the
 * builder, and both hold both keyhashes with certainty — each was pinned
 * at first contact (design §12.3) — so the ordering the wire already uses
 * for a pair settles it without a role and without a new mechanism. The
 * kernel orders the record's own fields by the same comparison
 * (`wire-format.md` §4.4), which is where that convention comes from.
 *
 * **No replication commitment is offered here.** `wire-format.md` §4.4
 * field 6 is optional and what it promises is bytes held for a peer
 * (design §3.4); a screen that asks for it is an operator's and not this
 * flow's, so this sends none.
 *
 * **The frames are this shell's own**, as [Adoption]'s are and for the
 * same reason, riding the end-to-end channel the prekey handover left
 * between the two devices under [KIND], a number outside the registry.
 */
class Peering(
    /** This device's keyhash, hex: which side builds turns on it. */
    val me: String,
    /** The counterparty's keyhash, hex. */
    val counterparty: String,
    /** What to call them on screen. */
    val counterpartyName: String,
    /** The presence record this rests on, hex. */
    val presence: String,
    private val kern: Kern,
    private val post: (ByteArray) -> Boolean,
) {

    /** The kernel calls a peering needs, as the shell holds them. */
    interface Kern {
        /** This archive's next back-pointers. */
        fun backPointers(): List<ByteArray>

        /** As the builder: the body, or the kernel's reason. `otherBack`
         *  is the other side's own back-pointers, which only they can
         *  supply. */
        fun propose(other: String, presence: String, otherBack: List<ByteArray>): Result<ByteArray>

        /** As the other side: the entries, or the reason it refused. */
        fun reviewAndSign(body: ByteArray): Result<ByteArray>

        /** The envelope over a body and both peers' entries. */
        fun envelope(body: ByteArray, entries: List<Pair<String, ByteArray>>): Result<ByteArray>

        /** Take the finished peering; the txid, or the reason. */
        fun take(envelope: ByteArray): Result<String>
    }

    /** What a call came to: a value, or the kernel's reason. */
    sealed class Result<out T> {
        data class Ok<T>(val value: T) : Result<T>()
        data class Refused(val reason: String) : Result<Nothing>()
    }

    /** Where the peering stands, for the screen. */
    enum class Step {
        /** The back-pointers are sent, or awaited. */
        OFFERED,

        /** The body is built and shown. */
        PROPOSED,

        /** The body is signed here, or its signature is held there. */
        SIGNED,

        /** The record is held. */
        HELD,

        /** Refused, with a reason; the meeting itself is untouched. */
        REFUSED,
    }

    /** Told whenever anything here moves, so a screen can re-render. */
    var onChanged: (() -> Unit)? = null

    private val lock = Any()
    private var step: Step = Step.OFFERED
    private var reason: String? = null
    private var txid: String? = null
    private var body: ByteArray? = null
    private val said = mutableListOf<String>()

    /** Whether this device builds the body: the lower keyhash does. */
    val builder: Boolean get() = me < counterparty

    fun step(): Step = synchronized(lock) { step }

    fun reason(): String? = synchronized(lock) { reason }

    /** The peering's txid once it is held. */
    fun txid(): String? = synchronized(lock) { txid }

    /** What has happened, for the screen's notice lines. */
    fun said(): List<String> = synchronized(lock) { said.toList() }

    private fun note(line: String) {
        synchronized(lock) {
            said.add(line)
            Diag.event("peer.note", "line" to Diag.scrub(line))
        }
        onChanged?.invoke()
    }

    private fun refuse(why: String) {
        synchronized(lock) {
            step = Step.REFUSED
            reason = why
            said.add(why)
            Diag.warn("peer.refused", "why" to Diag.scrub(why))
        }
        onChanged?.invoke()
    }

    /**
     * **Begin.** The side that does not build sends its back-pointers,
     * which is the one thing the builder cannot work out for itself; the
     * builder sends nothing and waits, having nothing to say until they
     * arrive.
     */
    fun begin() {
        if (builder) {
            note("waiting for ${counterpartyName}'s device to send what its archive chains to.")
            return
        }
        val back = kern.backPointers()
        if (back.isEmpty()) {
            refuse("this archive has no head to chain a peering to")
            return
        }
        if (!post(Frame.offer(back))) {
            refuse("the back-pointers could not be sent to $counterpartyName")
            return
        }
        note("sent what this archive chains to; $counterpartyName builds the record.")
    }

    /**
     * One frame from the counterparty. Everything it carries is checked by
     * the kernel; what this does is put each where it goes and keep the
     * person told.
     */
    fun frame(bytes: ByteArray) {
        val f = Frame.read(bytes)
        if (f == null) {
            refuse("a message of the peering did not read")
            return
        }
        when (f) {
            is Frame.Offer -> offered(f.back)
            is Frame.Body -> shown(f.body)
            is Frame.Signed -> countersigned(f.entries)
            is Frame.Refused -> refuse("${counterpartyName}'s device refused the body: ${f.why}")
            is Frame.Record -> recorded(f.envelope)
        }
    }

    /** As the builder: their back-pointers, then the body. */
    private fun offered(back: List<ByteArray>) {
        if (!builder) {
            refuse("back-pointers arrived at the side that does not build the body")
            return
        }
        when (val b = kern.propose(counterparty, presence, back)) {
            is Result.Refused -> refuse("this device would not propose the peering: ${b.reason}")
            is Result.Ok -> {
                synchronized(lock) {
                    body = b.value
                    step = Step.PROPOSED
                }
                if (!post(Frame.body(b.value))) {
                    refuse("the body could not be sent to $counterpartyName")
                    return
                }
                note("the body is with $counterpartyName for review.")
            }
        }
    }

    /** As the other side: review the body, sign it or refuse. */
    private fun shown(shown: ByteArray) {
        if (builder) {
            refuse("a body arrived at the side that builds it")
            return
        }
        when (val e = kern.reviewAndSign(shown)) {
            is Result.Refused -> {
                post(Frame.refused(e.reason))
                refuse("this device refused the body: ${e.reason}")
            }
            is Result.Ok -> {
                synchronized(lock) {
                    body = shown
                    step = Step.SIGNED
                }
                if (!post(Frame.signed(e.value))) {
                    refuse("the signature could not be sent to $counterpartyName")
                    return
                }
                note("the body checked out and is signed here.")
            }
        }
    }

    /** As the builder: both signatures, the record, and the copy they get. */
    private fun countersigned(theirs: ByteArray) {
        val b = synchronized(lock) { body }
        if (!builder || b == null) {
            refuse("a signature arrived with no body of this device's to put it over")
            return
        }
        when (val env = kern.envelope(b, listOf(counterparty to theirs))) {
            is Result.Refused -> refuse("the record would not assemble: ${env.reason}")
            is Result.Ok -> when (val t = kern.take(env.value)) {
                is Result.Refused -> refuse("this device would not take the record: ${t.reason}")
                is Result.Ok -> {
                    synchronized(lock) {
                        txid = t.value
                        step = Step.HELD
                    }
                    note("the peering is recorded: ${t.value.take(16)}…")
                    if (!post(Frame.record(env.value))) {
                        note("the record could not be sent to $counterpartyName; their device will want it before the edge is theirs too.")
                    }
                }
            }
        }
    }

    /** As the other side: the record, which is where the edge now is. */
    private fun recorded(envelope: ByteArray) {
        when (val t = kern.take(envelope)) {
            is Result.Refused -> refuse("the record did not check out here: ${t.reason}")
            is Result.Ok -> {
                synchronized(lock) {
                    txid = t.value
                    step = Step.HELD
                }
                note("the peering is held: ${t.value.take(16)}…")
            }
        }
    }

    /**
     * **The frames, versioned, this shell's own.** A version byte, a type
     * byte, then the type's own bytes — [Adoption.Frame]'s shape, for the
     * same reason, and a separate [KIND] so neither reads the other's.
     */
    object Frame {
        /** This framing's version. */
        const val VERSION: Byte = 1

        private const val OFFER: Byte = 0
        private const val BODY: Byte = 1
        private const val SIGNED: Byte = 2
        private const val REFUSED: Byte = 3
        private const val RECORD: Byte = 4

        sealed class Read
        data class Offer(val back: List<ByteArray>) : Read()
        data class Body(val body: ByteArray) : Read()
        data class Signed(val entries: ByteArray) : Read()
        data class Refused(val why: String) : Read()
        data class Record(val envelope: ByteArray) : Read()

        fun offer(back: List<ByteArray>): ByteArray {
            var out = byteArrayOf(VERSION, OFFER, back.size.toByte())
            back.forEach { out += it }
            return out
        }

        fun body(body: ByteArray): ByteArray = byteArrayOf(VERSION, BODY) + body

        fun signed(entries: ByteArray): ByteArray = byteArrayOf(VERSION, SIGNED) + entries

        fun refused(why: String): ByteArray = byteArrayOf(VERSION, REFUSED) + why.toByteArray()

        fun record(envelope: ByteArray): ByteArray = byteArrayOf(VERSION, RECORD) + envelope

        /** Whether these bytes are one of this shell's peering frames. */
        fun mine(bytes: ByteArray): Boolean =
            bytes.size >= 2 && bytes[0] == VERSION && bytes[1] in OFFER..RECORD

        fun read(bytes: ByteArray): Read? {
            if (!mine(bytes)) return null
            val rest = bytes.copyOfRange(2, bytes.size)
            return when (bytes[1]) {
                OFFER -> {
                    if (rest.isEmpty()) return null
                    val n = rest[0].toInt() and 0xff
                    if (n == 0 || rest.size != 1 + n * 32) return null
                    Offer((0 until n).map { i -> rest.copyOfRange(1 + i * 32, 33 + i * 32) })
                }
                BODY -> if (rest.isEmpty()) null else Body(rest)
                SIGNED -> if (rest.isEmpty()) null else Signed(rest)
                REFUSED -> Refused(String(rest))
                RECORD -> if (rest.isEmpty()) null else Record(rest)
                else -> null
            }
        }
    }

    companion object {
        /**
         * **The kind these frames ride under**, outside the registry
         * (`wire-format.md` §7.10.1: an unknown kind is application
         * payload to a recipient that does not know it), and distinct
         * from [Adoption.KIND] so a device never reads one as the other.
         * The shell's own number until the author rules on whether these
         * legs are registered kinds; nothing in any document claims it.
         */
        const val KIND: ULong = 1001UL
    }
}
