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
     * The largest message this will assemble. A carriage message is an
     * `IntentExchange` at the 256-entry ceiling or a `BundleContinuation`
     * of the same (`wire-format.md` §5.4), and a presented record runs to
     * ~65 KB (§12), so 256 of them is the bound that matters. **A peer that
     * announces more than this is refused rather than allocated for**: the
     * one thing a bearer can do to this side is ask it for memory.
     */
    const val MESSAGE_BOUND = 20 * 1024 * 1024

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
     * byte 1      flags: bit 0 set on the LAST packet of a message
     * bytes 2..3  the slice's index within its message, big-endian
     * bytes 4..   the slice
     * ```
     *
     * **The length is not carried and does not need to be**: the link
     * delivers packets, and the last one says so. A header this small keeps
     * the slice large on a link whose MTU is twenty-odd bytes, which is
     * what Bluetooth LE gives before negotiation.
     */
    const val HEADER = 4

    private const val LAST = 1

    /** Cut `messages` into packets for a link of `mtu`. */
    fun packets(messages: List<ByteArray>, mtu: Int): List<ByteArray> {
        require(mtu > HEADER) { "an MTU of $mtu carries no payload" }
        require(messages.size <= MESSAGES_BOUND) { "too many messages" }
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
                packet[1] = if (s == slices - 1) LAST.toByte() else 0
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
        private val slices = HashMap<Int, HashMap<Int, ByteArray>>()
        private val last = HashMap<Int, Int>()
        private var held = 0

        /** Why a packet was not taken, or null where it was. */
        fun take(packet: ByteArray): String? {
            if (packet.size < HEADER) return "a packet shorter than its header"
            // the index is one byte and MESSAGES_BOUND is 256, so every
            // value it can hold is a message this will assemble: there is
            // no out-of-range index to refuse, by construction
            val m = packet[0].toInt() and 0xff
            val s = ((packet[2].toInt() and 0xff) shl 8) or (packet[3].toInt() and 0xff)
            val slice = packet.copyOfRange(HEADER, packet.size)
            if (held + slice.size > MESSAGE_BOUND) return "more bytes than the bound allows"
            val into = slices.getOrPut(m) { HashMap() }
            // a repeat is dropped, not counted twice: a link may retry
            if (into.put(s, slice) == null) held += slice.size
            if (packet[1].toInt() and LAST != 0) last[m] = s
            return null
        }

        /**
         * The carriage set, or null while it is incomplete: `count`
         * messages, each whole, handed over in index order.
         */
        fun carriage(count: Int): List<ByteArray>? {
            if (count <= 0 || count > MESSAGES_BOUND) return null
            val out = ArrayList<ByteArray>(count)
            for (m in 0 until count) {
                val end = last[m] ?: return null
                val into = slices[m] ?: return null
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

    /** Move a whole carriage set over `link`. False where a packet failed. */
    fun carry(link: Link, messages: List<ByteArray>): Boolean =
        packets(messages, link.mtu()).all { link.send(it) }
}
