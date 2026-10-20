package com.comptus.fueros

/**
 * **The adoption after the record** (D7; design §6.1.1, where an adoption
 * carries both signatures, and §8.2, where it rests on a meeting).
 *
 * The presence record is what an adoption is evidence *of*, so this begins
 * where the ceremony ends: the two people have met, one of them is to be
 * the patron, and the direction is settled — either at D1a, or at D7 where
 * both chose the same side and were asked (PRD-05).
 *
 * **Five messages, and the kernel checks everything in them.** The
 * responder states the direction, being the only side that holds both
 * people's choices — its own from its D1a and the initiator's from the
 * bootstrap it read; the party being adopted then says which series it
 * can enter and what its archive's head is;
 * the patron builds the body and shows it; the other reviews it against
 * what it holds and signs or refuses with a reason; the patron assembles
 * the record, takes it, and sends it. Nothing here decides anything about
 * an adoption: `review_and_sign_adoption` is where a body is judged, and
 * `take_adoption` is where a record is.
 *
 * **The frames are this shell's own**, as the bootstrap object at D1 is,
 * and they ride the end-to-end channel the prekey handover left between
 * the two devices (`wire-format.md` §14.3.4) under [KIND], a number
 * outside the registry: to a client that does not know it, it is
 * application payload, which is §7.10.1's own rule for an unknown kind.
 *
 * **Owed to the author**: whether these legs should instead carry
 * registered kinds, so an adoption can be arranged between two parties
 * who are not face to face. Everything in them is the protocol's and
 * nothing is the shell's, which argues for registering them; the counter
 * argument is that an adoption rests on a meeting and the two are
 * therefore together, with a local session already open. Until that is
 * ruled, the number here is the shell's and no document claims it.
 */
class Adoption(
    /** Which side of the handshake this device was (`Meet.Role`): the
     *  responder settles the direction, the initiator is told it. */
    val role: Meet.Role,
    /** The counterparty's keyhash, hex. */
    val counterparty: String,
    /** What to call them on screen. */
    val counterpartyName: String,
    /** The presence record this rests on, hex. */
    val presence: String,
    private val kern: Kern,
    private val post: (ByteArray) -> Boolean,
) {

    /** The kernel calls an adoption needs, as the shell holds them. */
    interface Kern {
        /** A series this key has never occupied. */
        fun freshSeries(): Int?

        /** This archive's next back-pointers. */
        fun backPointers(): List<ByteArray>

        /** The scope this device brings under the patron, hex, or null
         *  where it joins as an individual (`wire-format.md` §4.1). */
        fun carrying(): String?

        /** Where this device sits, as a `Locator`, in the subnet it acts in. */
        fun position(): String

        /** As the patron: the body, or the kernel's reason. `carrying` is
         *  the scope the other brings, their claim, copied into the body. */
        fun propose(node: String, presence: String, series: Int, nodeBack: List<ByteArray>, carrying: String?): Result<ByteArray>

        /** As the party being adopted: the entries, or the reason it refused. */
        fun reviewAndSign(body: ByteArray): Result<ByteArray>

        /** The envelope over a body and the two signers' entries. */
        fun envelope(body: ByteArray, entries: List<Pair<String, ByteArray>>): Result<ByteArray>

        /** Take the finished adoption; the txid, or the reason. */
        fun take(envelope: ByteArray): Result<String>

        /** Where this device sits in `anchor`, in words, once it does. */
        fun positionIn(anchor: String): String?
    }

    /** What a call came to: a value, or the kernel's reason. */
    sealed class Result<out T> {
        data class Ok<T>(val value: T) : Result<T>()
        data class Refused(val reason: String) : Result<Nothing>()
    }

    /** Where the adoption stands, for the screen. */
    enum class Step {
        /** The party being adopted has said what it can take. */
        OFFERED,

        /** The patron has shown the body. */
        PROPOSED,

        /** The body is signed here, or its signature is held there. */
        SIGNED,

        /** The record is held and the position is real. */
        HELD,

        /** Refused, with a reason; the meeting itself is untouched. */
        REFUSED,
    }

    /** Told whenever anything here moves, so a screen can re-render. */
    var onChanged: (() -> Unit)? = null

    private val lock = Any()
    private var direction: Meet.Adopt? = null
    private var step: Step = Step.OFFERED
    private var reason: String? = null
    private var txid: String? = null
    private var into: String? = null
    private var body: ByteArray? = null
    private var mine: ByteArray? = null
    private val said = mutableListOf<String>()

    /** Whether this device is the patron, once the direction is settled. */
    val patron: Boolean
        get() = synchronized(lock) { direction == Meet.Adopt.THEM_UNDER_ME }

    /** The direction, as this device sees it, once it is settled. */
    fun direction(): Meet.Adopt? = synchronized(lock) { direction }

    /** The subnet the adoption is into, as the responder labelled it
     *  (`light-client-requirements.md` §7), once it has. */
    fun into(): String? = synchronized(lock) { into }

    fun step(): Step = synchronized(lock) { step }

    fun reason(): String? = synchronized(lock) { reason }

    /** The adoption's txid once it is held. */
    fun txid(): String? = synchronized(lock) { txid }

    /**
     * What has happened here, oldest first.
     *
     * **No screen draws these** [author, 2026-10-18]: each line reaches
     * the diagnostic stream as it is made, and what the screen shows is
     * where the adoption stands and what the person can do about it. The
     * list is the record a test asserts against.
     */
    fun said(): List<String> = synchronized(lock) { said.toList() }

    private fun note(line: String) {
        synchronized(lock) {
            said.add(line)
            Diag.event("adopt.note", "line" to Diag.scrub(line))
        }
        onChanged?.invoke()
    }

    private fun refuse(why: String) {
        synchronized(lock) {
            step = Step.REFUSED
            reason = why
            said.add(why)
            Diag.warn("adopt.refused", "why" to Diag.scrub(why))
        }
        onChanged?.invoke()
    }

    /**
     * **The responder states the direction and the two begin.**
     *
     * `settled` is what the two people's choices came to, in this
     * device's own frame: [Meet.Adopt.THEM_UNDER_ME] where this device is
     * the patron, [Meet.Adopt.ME_UNDER_THEM] where the other is, and
     * [Meet.Adopt.NONE] where they left the authority question alone —
     * which is a real answer and not a failure (PRD-05).
     *
     * Only the responder calls this: it read the initiator's choice off
     * the bootstrap and knows its own, so it is the only side that can
     * say what the two came to. The initiator learns it from the frame.
     */
    fun settle(settled: Meet.Adopt, label: String? = null) {
        check(role == Meet.Role.RESPONDER) { "the responder states the direction" }
        synchronized(lock) {
            direction = settled
            into = label
        }
        val told = when (settled) {
            Meet.Adopt.NONE -> 0
            Meet.Adopt.THEM_UNDER_ME -> 1
            Meet.Adopt.ME_UNDER_THEM -> 2
        }
        // and which subnet, in words, so the other side's acceptance can
        // state it (`light-client-requirements.md` §7)
        if (!post(Frame.direction(told, label))) {
            refuse("the direction could not be sent to $counterpartyName")
            return
        }
        if (settled == Meet.Adopt.NONE) {
            note("the authority question is left alone; the meeting stands on its own.")
            synchronized(lock) { step = Step.HELD }
            return
        }
        begin()
    }

    /**
     * **Begin**, once the direction is settled: the party being adopted
     * offers what it can take, which is the one thing its patron cannot
     * work out for itself — a series this key has never occupied
     * (`light-client-requirements.md` §2) and the head its signature will
     * chain to.
     *
     * The patron sends nothing and waits: it has nothing to say until the
     * offer arrives.
     */
    fun begin() {
        if (direction() == null) {
            note("waiting for ${counterpartyName}'s device to say which way the adoption runs.")
            return
        }
        if (patron) {
            note("waiting for ${counterpartyName}'s device to say which series it can enter.")
            return
        }
        val series = kern.freshSeries()
        if (series == null) {
            refuse("this key has occupied every series it could be adopted into")
            return
        }
        val back = kern.backPointers()
        val carrying = kern.carrying()
        if (!post(Frame.offer(series, back, carrying))) {
            refuse("the offer could not be sent to ${counterpartyName}")
            return
        }
        note(
            "offered series $series to $counterpartyName" +
                (carrying?.let { ", bringing scope ${it.take(8)}…" } ?: ", as an individual") + ".",
        )
    }

    /**
     * One frame from the counterparty. Everything it carries is checked by
     * the kernel; what this does is put each where it goes and keep the
     * person told.
     */
    fun frame(bytes: ByteArray) {
        val f = Frame.read(bytes)
        if (f == null) {
            refuse("a message of the adoption did not read")
            return
        }
        when (f) {
            is Frame.Direction -> told(f)
            is Frame.Offer -> offered(f)
            is Frame.Body -> shown(f.body)
            is Frame.Signed -> countersigned(f.entries)
            is Frame.Refused -> refuse("${counterpartyName}'s device refused the body: ${f.why}")
            is Frame.Record -> recorded(f.envelope)
        }
    }

    /**
     * As the initiator: the direction the responder settled, in *their*
     * frame, so it is mirrored into this device's.
     */
    private fun told(d: Frame.Direction) {
        if (role != Meet.Role.INITIATOR) {
            refuse("a direction arrived at the side that states it")
            return
        }
        val mirrored = when (d.who) {
            1 -> Meet.Adopt.ME_UNDER_THEM
            2 -> Meet.Adopt.THEM_UNDER_ME
            else -> Meet.Adopt.NONE
        }
        synchronized(lock) {
            direction = mirrored
            into = d.label
        }
        if (mirrored == Meet.Adopt.NONE) {
            note("$counterpartyName left the authority question alone; the meeting stands on its own.")
            synchronized(lock) { step = Step.HELD }
            return
        }
        note(
            if (mirrored == Meet.Adopt.THEM_UNDER_ME) {
                "$counterpartyName asks to be adopted here" + (d.label?.let { ", into $it" } ?: "") + "."
            } else {
                "$counterpartyName will be the patron" + (d.label?.let { ", in $it" } ?: "") + "."
            },
        )
        begin()
    }

    /** As the patron: the offer, and the body it allows. */
    private fun offered(o: Frame.Offer) {
        if (!patron) {
            refuse("an offer arrived at the party being adopted")
            return
        }
        note(
            "$counterpartyName can enter series ${o.series}" +
                (o.carrying?.let { ", bringing scope ${it.take(8)}…" } ?: "") + ".",
        )
        when (val b = kern.propose(counterparty, presence, o.series, o.back, o.carrying)) {
            is Result.Refused -> refuse("this device would not propose the adoption: ${b.reason}")
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

    /** As the party being adopted: review the body, sign it or refuse. */
    private fun shown(shown: ByteArray) {
        if (patron) {
            refuse("a body arrived at the patron, which is the side that builds it")
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
                    mine = e.value
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

    /** As the patron: both signatures, the record, and the copy they get. */
    private fun countersigned(theirs: ByteArray) {
        val b = synchronized(lock) { body }
        if (!patron || b == null) {
            refuse("a signature arrived with no body of this device's to put it over")
            return
        }
        val entries = listOf(counterparty to theirs)
        when (val env = kern.envelope(b, entries)) {
            is Result.Refused -> refuse("the record would not assemble: ${env.reason}")
            is Result.Ok -> {
                when (val t = kern.take(env.value)) {
                    is Result.Refused -> refuse("this device would not take the record: ${t.reason}")
                    is Result.Ok -> {
                        synchronized(lock) {
                            txid = t.value
                            step = Step.HELD
                        }
                        note("the adoption is recorded: ${t.value.take(16)}…")
                        if (!post(Frame.record(env.value))) {
                            note("the record could not be sent to $counterpartyName; their device will want it before it can act in this subnet.")
                        }
                    }
                }
            }
        }
    }

    /** As the party being adopted: the record, which is where it now sits. */
    private fun recorded(envelope: ByteArray) {
        when (val t = kern.take(envelope)) {
            is Result.Refused -> refuse("the record did not check out here: ${t.reason}")
            is Result.Ok -> {
                synchronized(lock) {
                    txid = t.value
                    step = Step.HELD
                }
                note("the adoption is held: ${t.value.take(16)}…")
            }
        }
    }

    /**
     * Where this device sits now, in the subnet it acts in: for the
     * patron the one it issued in, for the other the one it was put in,
     * which is the same subnet and the kernel's answer either way.
     */
    fun position(): String = kern.position()

    /**
     * **The frames, versioned, this shell's own.** A version byte, a type
     * byte, then the type's own bytes: the same shape the bootstrap uses,
     * for the same reason — a later framing can differ and be told apart.
     */
    object Frame {
        /** This framing's version. */
        const val VERSION: Byte = 1

        private const val DIRECTION: Byte = 0
        private const val OFFER: Byte = 1
        private const val BODY: Byte = 2
        private const val SIGNED: Byte = 3
        private const val REFUSED: Byte = 4
        private const val RECORD: Byte = 5

        sealed class Read
        data class Direction(val who: Int, val label: String?) : Read()
        data class Offer(val series: Int, val back: List<ByteArray>, val carrying: String?) : Read()
        data class Body(val body: ByteArray) : Read()
        data class Signed(val entries: ByteArray) : Read()
        data class Refused(val why: String) : Read()
        data class Record(val envelope: ByteArray) : Read()

        fun direction(who: Int, label: String? = null): ByteArray =
            byteArrayOf(VERSION, DIRECTION, who.toByte()) + (label ?: "").toByteArray()

        fun offer(series: Int, back: List<ByteArray>, carrying: String? = null): ByteArray {
            val head = byteArrayOf(
                VERSION,
                OFFER,
                (series ushr 24).toByte(),
                (series ushr 16).toByte(),
                (series ushr 8).toByte(),
                series.toByte(),
                back.size.toByte(),
            )
            var out = head
            back.forEach { out += it }
            // the scope brought, 32 bytes after the lists, where one is
            carrying?.let { out += Provision.unhex(it) }
            return out
        }

        fun body(body: ByteArray): ByteArray = byteArrayOf(VERSION, BODY) + body

        fun signed(entries: ByteArray): ByteArray = byteArrayOf(VERSION, SIGNED) + entries

        fun refused(why: String): ByteArray = byteArrayOf(VERSION, REFUSED) + why.toByteArray()

        fun record(envelope: ByteArray): ByteArray = byteArrayOf(VERSION, RECORD) + envelope

        /** Whether these bytes are one of this shell's adoption frames. */
        fun mine(bytes: ByteArray): Boolean =
            bytes.size >= 2 && bytes[0] == VERSION && bytes[1] in DIRECTION..RECORD

        fun read(bytes: ByteArray): Read? {
            if (!mine(bytes)) return null
            val rest = bytes.copyOfRange(2, bytes.size)
            return when (bytes[1]) {
                DIRECTION -> if (rest.isEmpty()) null else Direction(rest[0].toInt(), String(rest.copyOfRange(1, rest.size)).ifEmpty { null })
                OFFER -> {
                    if (rest.size < 5) return null
                    val series = (rest[0].toInt() and 0xff shl 24) or
                        (rest[1].toInt() and 0xff shl 16) or
                        (rest[2].toInt() and 0xff shl 8) or
                        (rest[3].toInt() and 0xff)
                    val n = rest[4].toInt() and 0xff
                    val lists = 5 + n * 32
                    if (rest.size != lists && rest.size != lists + 32) return null
                    val back = (0 until n).map { i -> rest.copyOfRange(5 + i * 32, 37 + i * 32) }
                    val carrying = if (rest.size == lists + 32) Provision.hex(rest.copyOfRange(lists, lists + 32)) else null
                    Offer(series, back, carrying)
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
         * payload to a recipient that does not know it). The shell's own
         * number until the author rules on whether the adoption's legs
         * are registered kinds; nothing in any document claims it.
         */
        const val KIND: ULong = 1000UL
    }
}
