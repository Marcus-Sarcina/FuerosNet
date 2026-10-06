package com.comptus.fueros

/**
 * One carriage, over one bearer (`wire-format.md` §14.3.1).
 *
 * This is what the Meet flow calls: hand it the kernel's carriage set, and
 * it moves it and tells you what came back. It owns the **choice** of
 * bearer and the **ordering** of the exchange, and it owns neither the
 * bytes nor their meaning.
 *
 * **The hierarchy is §14.3.1's and the reason is locality.** A direct local
 * radio first; a fetch over FuerosNet last, because that one leaves the
 * physical-presence property §14.1 names — the two devices reach each
 * other through nodes rather than across the room. [Chosen] records which
 * was used, because the person is owed that: a ceremony carried over
 * infrastructure is a weaker thing than one carried across a table, and
 * the record does not say which.
 *
 * **What this is not.** It is not integrity. Every byte it moves is already
 * bound to the optical anchor, and the kernel checks that binding when it
 * takes the bytes back. A bearer that reorders, repeats, truncates or
 * forges produces a message the kernel refuses — so the only thing this
 * file owes is to assemble what was actually sent, in order, or nothing.
 */
class Carriage(private val radio: Bearer.Link?) {

    /**
     * **The phases, named once.** Each is one exchange of the ceremony's
     * conversation over the bearer, and the number is what the packet
     * header carries — so a late packet of one phase cannot be assembled
     * into another, and a receiver needs no guesswork about what a set of
     * bytes was.
     *
     * Every phase carries `wire-format.md` §14.3.2's encodings, opaque
     * here: INTENT the intent and its continuations, PROXIMITY the
     * `ProximityOutcomes`, CAPTURE_KEY the `CaptureKeyHandover` design
     * §7.5.2.6 requires. Each is anchored to the ceremony-id and the
     * kernel checks the anchor when it takes the bytes back; the phase
     * number is only this shell's framing, and keeps one phase's packets
     * out of another's assembly.
     */
    object Phase {
        /** The intent exchange and any bundle continuations with it. */
        const val INTENT = 0
        /** What the distance channels measured. */
        const val PROXIMITY = 1
        /** The capture key, one each way. */
        const val CAPTURE_KEY = 2

        /**
         * The conversation's carriages, from the capture on: the
         * counterparty's leg of kinds 9 to 18 crosses this bearer
         * (design §7.1), sealed under the local session, and **each
         * message takes a phase of its own** from here to the last the
         * header can name. A phase's set is whole only once its sender
         * has flagged a last message, and the conversation does not know
         * how many it will send, so one message per phase is what lets
         * each be taken the moment it is in. The receiver discards a
         * phase as it takes it, so the sender's count wraps round the
         * range without colliding with anything still waiting.
         */
        const val CONVERSATION = 3

        /** How many phases the conversation has to wrap through. */
        const val CONVERSATIONS = Bearer.PHASES - CONVERSATION

        /** The phase the `n`th conversation carriage this side sends
         *  goes as, counted from zero. */
        fun conversation(n: Int): Int = CONVERSATION + Math.floorMod(n, CONVERSATIONS)
    }

    /** Which bearer a carriage runs over. */
    enum class Chosen {
        /** The radio between the two devices. */
        RADIO,
        /** §14.3.1's last resort, a fetch through the network. Not built. */
        FETCH,
        /** Nothing to carry over. */
        NONE,
    }

    /** Which bearer this would use, given what is to hand. */
    val chosen: Chosen = when {
        radio != null -> Chosen.RADIO
        // §14.3.1's last resort. NOT BUILT: a fetch needs the counterparty
        // reachable through the network, which before a ceremony they may
        // not be, and the kernel's payload path needs a session this
        // exchange is what establishes. Named here so the gap is visible
        // rather than implied.
        else -> Chosen.NONE
    }

    private val inward = Bearer.Reassembly()

    /**
     * One phase's traffic, for the diagnostics (`Robot/field-test-
     * diagnostics.md`, section 3.6, `bearer.packets`): what was sent and
     * what arrived, the repeats, the refusals by reason, and the time from
     * the first packet to the set being whole. Counts, sizes and durations;
     * nothing of what the packets carried.
     */
    class Counters {
        /** Packets sent. */
        var sent = 0
        /** Their bytes. */
        var sentBytes = 0
        /** Sends the radio refused. */
        var sendFailed = 0
        /** Packets taken. */
        var received = 0
        /** Their bytes. */
        var receivedBytes = 0
        /** Repeats dropped. */
        var duplicates = 0
        /** Refusals by reason, in the order each reason first occurred. */
        val refused = linkedMapOf<String, Int>()
        /** When the first packet of the phase arrived. */
        var firstMs: Long? = null
        /** When the set became whole. */
        var wholeMs: Long? = null
        /** Whether the event for this phase has gone out. */
        var reported = false

        /** How many packets were refused, over every reason. */
        fun refusals(): Int = refused.values.sum()

        /** The event's fields. */
        fun fields(phase: Int): List<Pair<String, Any?>> = listOf(
            "phase" to phase,
            "sent" to sent,
            "sent_bytes" to sentBytes,
            "send_failed" to sendFailed,
            "received" to received,
            "received_bytes" to receivedBytes,
            "duplicates" to duplicates,
            "refused" to refusals(),
            "refused_why" to refused.takeIf { it.isNotEmpty() },
            "assembly_ms" to firstMs?.let { f -> wholeMs?.let { it - f } },
        )
    }

    private val counters = HashMap<Int, Counters>()

    /** The traffic one phase has seen so far. */
    fun counters(phase: Int): Counters = synchronized(counters) { counters.getOrPut(phase) { Counters() } }

    /** What the radio delivered, packet by packet. */
    fun packet(bytes: ByteArray): String? {
        // the phase nibble, read here only to file the count: a packet too
        // short to carry one is refused below and counted against none
        val phase = if (bytes.size >= Bearer.HEADER) (bytes[1].toInt() ushr 4) and 0x0f else -1
        val before = inward.taken
        val dup = inward.duplicates
        val why = inward.take(bytes)
        val c = counters(phase)
        synchronized(c) {
            if (c.firstMs == null) c.firstMs = Diag.ms()
            if (why != null) {
                c.refused[why] = (c.refused[why] ?: 0) + 1
                Diag.warn("bearer.refused", "phase" to phase, "reason" to why)
            } else if (inward.duplicates > dup) {
                c.duplicates += 1
            } else if (inward.taken > before) {
                c.received += 1
                c.receivedBytes += bytes.size - Bearer.HEADER
            }
        }
        return why
    }

    /**
     * Send `messages` in the order given, as `phase`. False where a packet
     * did not go, which is unsent work and not a delivery. With `wipe`,
     * the packets cut from `messages` are zeroed once sent: what the
     * CAPTURE_KEY phase carries is not kept anywhere on this side.
     */
    fun send(messages: List<ByteArray>, phase: Int = Phase.INTENT, wipe: Boolean = false): Boolean {
        val link = radio ?: return false
        val c = counters(phase)
        val counting = object : Bearer.Link {
            override fun mtu(): Int = link.mtu()
            override fun send(packet: ByteArray): Boolean {
                val ok = link.send(packet)
                synchronized(c) {
                    if (ok) {
                        c.sent += 1
                        c.sentBytes += packet.size - Bearer.HEADER
                    } else {
                        c.sendFailed += 1
                    }
                }
                return ok
            }
        }
        val started = Diag.ms()
        val ok = Bearer.carry(counting, messages, phase, wipe)
        Diag.event(
            "bearer.packets",
            "dir" to "sent",
            "phase" to phase,
            "messages" to messages.size,
            "packets" to c.sent,
            "bytes" to c.sentBytes,
            "failed" to c.sendFailed,
            "ok" to ok,
            "ms" to (Diag.ms() - started),
        )
        return ok
    }

    /**
     * Forget what `phase` carried, wiped. [received] copies out of the
     * assembly and leaves it standing; once a phase has been taken, this
     * is how the assembly's own copy goes too.
     */
    fun discard(phase: Int) {
        report(phase)
        inward.discard(phase)
    }

    /** The phase's inbound tally, once: when its set is whole, or when it
     *  is discarded without ever being. */
    private fun report(phase: Int) {
        val c = counters(phase)
        val fields = synchronized(c) {
            if (c.reported) return
            c.reported = true
            c.fields(phase)
        }
        Diag.event("bearer.packets", *(listOf("dir" to "received") + fields).toTypedArray())
    }

    /**
     * The counterparty's carriage set once it is whole, or null while it is
     * not. **Nobody is told how many messages to expect**: the sender flags
     * its last one, and a set missing that flag is incomplete rather than
     * short.
     */
    fun received(phase: Int = Phase.INTENT): List<ByteArray>? {
        val set = inward.carriage(phase) ?: return null
        val c = counters(phase)
        synchronized(c) { if (c.wholeMs == null) c.wholeMs = Diag.ms() }
        report(phase)
        return set
    }
}
