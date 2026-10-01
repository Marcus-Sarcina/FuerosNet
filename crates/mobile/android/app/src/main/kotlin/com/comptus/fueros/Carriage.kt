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
     * Send `messages` in the order given. False where a packet did not go,
     * which is unsent work and not a delivery.
     */
    fun send(messages: List<ByteArray>): Boolean {
        val link = radio ?: return false
        return Bearer.carry(link, messages)
    }

    /**
     * The counterparty's carriage set once `count` messages are whole, or
     * null while they are not.
     *
     * `count` comes from the kernel, which reads it out of the
     * `IntentExchange`'s continuation field — so the number of messages to
     * expect is the sender's claim, checked by whether they assemble.
     */
    fun received(count: Int): List<ByteArray>? = inward.carriage(count)
}
