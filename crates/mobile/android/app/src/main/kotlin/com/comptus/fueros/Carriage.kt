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
        const val INTENT = 0
        const val PROXIMITY = 1
        const val CAPTURE_KEY = 2
    }

    enum class Chosen { RADIO, FETCH, NONE }

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

    /** What the radio delivered, packet by packet. */
    fun packet(bytes: ByteArray): String? = inward.take(bytes)

    /**
     * Send `messages` in the order given, as `phase`. False where a packet
     * did not go, which is unsent work and not a delivery. With `wipe`,
     * the packets cut from `messages` are zeroed once sent: what the
     * CAPTURE_KEY phase carries is not kept anywhere on this side.
     */
    fun send(messages: List<ByteArray>, phase: Int = Phase.INTENT, wipe: Boolean = false): Boolean {
        val link = radio ?: return false
        return Bearer.carry(link, messages, phase, wipe)
    }

    /**
     * Forget what `phase` carried, wiped. [received] copies out of the
     * assembly and leaves it standing; once a phase has been taken, this
     * is how the assembly's own copy goes too.
     */
    fun discard(phase: Int) = inward.discard(phase)

    /**
     * The counterparty's carriage set once it is whole, or null while it is
     * not. **Nobody is told how many messages to expect**: the sender flags
     * its last one, and a set missing that flag is incomplete rather than
     * short.
     */
    fun received(phase: Int = Phase.INTENT): List<ByteArray>? = inward.carriage(phase)
}
