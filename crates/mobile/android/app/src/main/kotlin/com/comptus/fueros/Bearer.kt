package com.comptus.fueros

/**
 * The bearer, as this shell moves a carriage over one
 * (`wire-format.md` §14.3.1).
 *
 * **What ranks a bearer is locality, not integrity.** §14.3.1 puts a direct
 * local radio first and a fetch over FuerosNet last, and says why: the
 * integrity comes from the optical anchor, so a bearer is preferred for
 * keeping the exchange off infrastructure and inside the physical-presence
 * property §14.1 names. **Nothing here is trusted.** A bearer that
 * reorders, truncates, duplicates or forges is caught by the kernel's own
 * checks — the echoed contribution and the anchor — and a message this file
 * assembles wrongly is a message the kernel refuses.
 *
 * So this is a transport and reads nothing it carries. What it owes its
 * caller is only this: the messages that arrive are the messages that were
 * sent, **in the order they were sent**, or they do not arrive at all. A
 * carriage set is ordered (§14.3.2: the exchange, then continuations
 * numbered from one), and handing the kernel a set it cannot trust the
 * order of would make it refuse a bundle that was whole.
 */
object Bearer {

    /**
     * Wait, bounded, for a link to come ready: `ready` is asked every
     * `stepMs` until it says so or `budgetMs` has passed, and the answer is
     * whether it did. A radio's offer or seek returns as the radio starts,
     * not as a peer connects and subscribes, and a packet sent before then
     * goes nowhere and reports success; so every phase waits here first.
     * The clock and the pause are parameters so a test can run it without
     * sleeping.
     */
    fun awaitReady(
        budgetMs: Long,
        stepMs: Long = 100,
        now: () -> Long = { System.nanoTime() / 1_000_000 },
        pause: (Long) -> Unit = { Thread.sleep(it) },
        ready: () -> Boolean,
    ): Boolean {
        val deadline = now() + budgetMs
        while (true) {
            if (ready()) return true
            if (now() >= deadline) return false
            pause(stepMs)
        }
    }
    /**
     * The most this will hold for one ceremony's carriages: the slices'
     * bytes **plus what holding each one costs** ([ENTRY_COST]). A carriage
     * message is an `IntentExchange` at the 256-entry ceiling or a
     * `BundleContinuation` of the same (`wire-format.md` §5.4), and a
     * presented record runs to ~65 KB (§12), so 256 of them — some 17 MB —
     * is the payload that matters, and the rest is room for the entries
     * that index it at a negotiated MTU. **A peer that asks for more is
     * refused rather than allocated for**: the one thing a bearer can do
     * to this side is ask it for memory.
     */
    const val MESSAGE_BOUND = 24 * 1024 * 1024

    /**
     * What one held slice costs beyond its bytes: the map entry that
     * indexes it and the array that holds it, as a JVM carries them.
     * **Counted because a slice of no bytes is not free.** A peer sending
     * empty or one-byte slices at fresh indices — 16 phases, 256 messages,
     * 65,536 slots each — would otherwise grow the maps without touching a
     * bound that counted bytes alone. The figure is an estimate; what
     * matters is that it is not zero.
     */
    const val ENTRY_COST = 64

    /**
     * Messages in one carriage set: an exchange and its continuations.
     *
     * **256, because the header indexes them in one byte** — and 256
     * carriages at 256 entries each is 65,536 bundle entries, where §5.4's
     * own arithmetic has a subject reaching 256 at about one meeting every
     * three days across the 730-day window. A bundle that needed a
     * 257th carriage would be two orders of magnitude past anything the
     * design contemplates, and a second header byte spent against it would
     * come out of the slice on a link whose MTU is twenty.
     */
    const val MESSAGES_BOUND = 256

    /** A link that moves opaque packets, and promises nothing about them. */
    interface Link {
        /** The most bytes one packet may carry. */
        fun mtu(): Int

        /** Send one packet. False where it did not go. */
        fun send(packet: ByteArray): Boolean
    }

    /**
     * What a packet is: a 4-byte header then the slice.
     *
     * ```
     * byte 0      message index, 0..=MESSAGES_BOUND-1
     * byte 1      bits 0..3 flags: bit 0 set on the LAST packet of a message
     *                              bit 1 set on the last packet of the LAST
     *                              message
     *             bits 4..7 the PHASE this packet belongs to, 0..=15
     * bytes 2..3  the slice's index within its message, big-endian
     * bytes 4..   the slice
     * ```
     *
     * **A phase is one exchange of the ceremony's conversation** — the
     * intent carriage, then the proximity outcomes, then the capture key —
     * and the phase bits are what keep a late packet of one from being
     * assembled into another. The phases' *meaning* is fixed by [Carriage],
     * not here: this file moves packets and reads nothing.
     *
     * **The length is not carried and does not need to be**: the link
     * delivers packets, and the last one says so. A header this small keeps
     * the slice large on a link whose MTU is twenty-odd bytes, which is
     * what Bluetooth LE gives before negotiation.
     *
     * **Bit 1 is why the receiver need not be told the count.** A carriage
     * set is as long as the bundle makes it (`wire-format.md` §5.4), and a
     * receiver that had to be told how many messages to expect would be
     * taking the sender's word for it in a second place. The end flag says
     * *this was the last*, and a set is complete when that message and
     * every message before it is whole — so a truncated set is an
     * incomplete one rather than a short one mistaken for all of it.
     */
    const val HEADER = 4

    private const val LAST = 1
    private const val END = 2

    /** Phases one link can tell apart: the header spends a nibble. */
    const val PHASES = 16

    /** Cut `messages` into packets for a link of `mtu`, as `phase`. */
    fun packets(messages: List<ByteArray>, mtu: Int, phase: Int = 0): List<ByteArray> {
        require(mtu > HEADER) { "an MTU of $mtu carries no payload" }
        require(messages.size <= MESSAGES_BOUND) { "too many messages" }
        require(phase in 0 until PHASES) { "a phase is one nibble" }
        val room = mtu - HEADER
        val out = ArrayList<ByteArray>()
        for ((m, message) in messages.withIndex()) {
            // a zero-length message is still a message, and still arrives
            val slices = maxOf(1, (message.size + room - 1) / room)
            for (s in 0 until slices) {
                val from = s * room
                val to = minOf(message.size, from + room)
                val slice = message.copyOfRange(from, to)
                val packet = ByteArray(HEADER + slice.size)
                packet[0] = m.toByte()
                val lastSlice = s == slices - 1
                val lastMessage = m == messages.size - 1
                val flags = when {
                    lastSlice && lastMessage -> LAST or END
                    lastSlice -> LAST
                    else -> 0
                }
                packet[1] = (flags or (phase shl 4)).toByte()
                packet[2] = (s ushr 8).toByte()
                packet[3] = s.toByte()
                slice.copyInto(packet, HEADER)
                out.add(packet)
            }
        }
        return out
    }

    /**
     * The receiving side: packets in, whole messages out, in order.
     *
     * **A message completes only when every slice before its last has
     * arrived.** A gap is not guessed at and not waited out here — the
     * caller is told nothing is ready, and the ceremony's own timeout is
     * what ends it. Assembling across a gap would hand the kernel bytes
     * that are not what was sent, and the kernel's refusal would then read
     * as the counterparty's fault.
     */
    class Reassembly {
        /** One phase's assembly state, apart from every other's. */
        private class Phase {
            val slices = HashMap<Int, HashMap<Int, ByteArray>>()
            val last = HashMap<Int, Int>()
            var end: Int? = null
        }

        private val phases = HashMap<Int, Phase>()
        private var held = 0

        /**
         * What has passed through, for the diagnostics: packets taken,
         * their slice bytes, and repeats dropped (`Robot/field-test-
         * diagnostics.md`, section 3.6, `bearer.packets`). Counts and
         * sizes, never contents.
         */
        var taken = 0
            private set
        var takenBytes = 0
            private set
        var duplicates = 0
            private set

        /** Why a packet was not taken, or null where it was. */
        fun take(packet: ByteArray): String? {
            if (packet.size < HEADER) return "a packet shorter than its header"
            // the index is one byte and MESSAGES_BOUND is 256, so every
            // value it can hold is a message this will assemble: there is
            // no out-of-range index to refuse, by construction — and the
            // phase is a nibble, bounded the same way
            val m = packet[0].toInt() and 0xff
            val s = ((packet[2].toInt() and 0xff) shl 8) or (packet[3].toInt() and 0xff)
            val flags = packet[1].toInt()
            val last = flags and LAST != 0
            val slice = packet.copyOfRange(HEADER, packet.size)
            // [packets] cuts a message into full slices and one last one, so
            // an empty slice that ends nothing was never sent: it is refused
            // before it can cost an entry
            if (slice.isEmpty() && !last) return "an empty slice that ends nothing"
            val ph = phases.getOrPut((flags ushr 4) and 0x0f) { Phase() }
            // nothing follows a message's last slice, and a message has one
            ph.last[m]?.let { end ->
                if (s > end) return "a slice past the message's last"
                if (last && s != end) return "a second last slice"
            }
            if (held + slice.size + ENTRY_COST > MESSAGE_BOUND) return "more bytes than the bound allows"
            val into = ph.slices.getOrPut(m) { HashMap() }
            // a repeat is dropped, not counted twice: a link may retry
            if (into.put(s, slice) == null) {
                held += slice.size + ENTRY_COST
                taken += 1
                takenBytes += slice.size
            } else {
                duplicates += 1
            }
            if (last) ph.last[m] = s
            if (flags and END != 0) ph.end = m
            return null
        }

        /**
         * The whole carriage set, or null while it is incomplete: every
         * message up to and including the one flagged as last, each whole,
         * in index order.
         *
         * **Nobody says how many to expect.** The sender flags its last
         * message and this waits for that one and everything before it.
         */
        fun carriage(phase: Int = 0): List<ByteArray>? {
            val ph = phases[phase] ?: return null
            val last = ph.end ?: return null
            return carriage(phase, last + 1)
        }

        /**
         * Forget one phase's slices, each wiped first. What a phase carried
         * is the caller's once taken, and a key among it is not kept here:
         * the CAPTURE_KEY phase carries one, and this is how the shell lets
         * it go (design §7.5.2).
         */
        fun discard(phase: Int) {
            val ph = phases.remove(phase) ?: return
            for (into in ph.slices.values) for (slice in into.values) {
                slice.fill(0)
                held -= slice.size + ENTRY_COST
            }
        }

        /**
         * The same, for a count known another way. A set whose length the
         * caller already knows is checked against it.
         */
        fun carriage(phase: Int, count: Int): List<ByteArray>? {
            if (count <= 0 || count > MESSAGES_BOUND) return null
            val ph = phases[phase] ?: return null
            val out = ArrayList<ByteArray>(count)
            for (m in 0 until count) {
                val end = ph.last[m] ?: return null
                val into = ph.slices[m] ?: return null
                if (into.size != end + 1) return null
                val whole = ArrayList<Byte>()
                for (s in 0..end) {
                    val slice = into[s] ?: return null
                    for (b in slice) whole.add(b)
                }
                out.add(whole.toByteArray())
            }
            return out
        }
    }

    /**
     * Move a whole carriage set over `link`. False where a packet failed.
     * With `wipe`, every packet is zeroed once the link has had it: a
     * packet is this file's own copy of what it carries, and a carriage
     * that carries a key leaves no copy behind here.
     */
    fun carry(link: Link, messages: List<ByteArray>, phase: Int = 0, wipe: Boolean = false): Boolean {
        val out = packets(messages, link.mtu(), phase)
        return try {
            out.all { link.send(it) }
        } finally {
            if (wipe) out.forEach { it.fill(0) }
        }
    }
}
