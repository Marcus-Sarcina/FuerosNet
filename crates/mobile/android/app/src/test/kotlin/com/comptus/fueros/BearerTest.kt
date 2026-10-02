package com.comptus.fueros

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The bearer's framing, against a link that does what links do: a small
 * MTU, reordering, repeats, and loss.
 *
 * **Nothing here is trusted and the tests are written that way.** What the
 * bearer owes is that the messages arriving are the messages sent, in
 * order, or that nothing arrives — because a carriage set is ordered
 * (`wire-format.md` §14.3.2) and a set assembled wrongly would make the
 * kernel refuse a bundle that was whole.
 */
class BearerTest {

    /** A link that keeps what it was given. */
    private class Wire(val mtu: Int) : Bearer.Link {
        val sent = ArrayList<ByteArray>()
        override fun mtu() = mtu
        override fun send(packet: ByteArray): Boolean = sent.add(packet)
    }

    private fun carriage(vararg sizes: Int) =
        sizes.mapIndexed { m, n -> ByteArray(n) { (it * 13 + m).toByte() } }

    @Test
    fun a_carriage_crosses_a_twenty_byte_link_and_arrives_in_order() {
        // twenty is what Bluetooth LE gives before any negotiation, and
        // the header leaves sixteen bytes of slice
        val wire = Wire(20)
        val messages = carriage(53, 36, 1000, 0)
        assertTrue(Bearer.carry(wire, messages))
        val r = Bearer.Reassembly()
        for (p in wire.sent) assertNull(r.take(p))
        val back = r.carriage(0)
        assertNotNull(back)
        assertEquals(messages.size, back!!.size)
        for (i in messages.indices) {
            assertArrayEquals("message $i", messages[i], back[i])
        }
    }

    @Test
    fun order_on_the_link_does_not_decide_order_in_the_carriage() {
        val wire = Wire(24)
        val messages = carriage(300, 90)
        Bearer.carry(wire, messages)
        val r = Bearer.Reassembly()
        // every packet, backwards, and twice: a link may reorder and retry
        for (p in wire.sent.reversed()) assertNull(r.take(p))
        for (p in wire.sent.reversed()) assertNull(r.take(p))
        val back = r.carriage(0)!!
        assertArrayEquals(messages[0], back[0])
        assertArrayEquals(messages[1], back[1])
    }

    @Test
    fun a_gap_yields_nothing_rather_than_bytes_that_were_not_sent() {
        val wire = Wire(20)
        val messages = carriage(500)
        Bearer.carry(wire, messages)
        val r = Bearer.Reassembly()
        // everything but one slice in the middle
        for ((i, p) in wire.sent.withIndex()) if (i != 3) assertNull(r.take(p))
        assertNull("a gap is not assembled across", r.carriage(0, 1))
        // and the moment it arrives, the message is whole
        assertNull(r.take(wire.sent[3]))
        assertArrayEquals(messages[0], r.carriage(0, 1)!![0])
    }

    @Test
    fun a_message_whose_last_packet_never_came_is_not_complete() {
        val wire = Wire(20)
        Bearer.carry(wire, carriage(200))
        val r = Bearer.Reassembly()
        for (p in wire.sent.dropLast(1)) assertNull(r.take(p))
        assertNull("no last packet, no message", r.carriage(0, 1))
    }

    @Test
    fun a_peer_asking_for_memory_is_refused_rather_than_allocated_for() {
        val r = Bearer.Reassembly()
        // a packet shorter than its own header
        assertEquals("a packet shorter than its header", r.take(ByteArray(2)))
        // every value the one-byte index can hold is a message this will
        // assemble, so there is no out-of-range index to refuse: the bound
        // that matters is on bytes held, below
        val top = ByteArray(Bearer.HEADER + 1)
        top[0] = 0xff.toByte()
        top[1] = 1
        assertNull(r.take(top))
        assertEquals(Bearer.MESSAGES_BOUND, 256)
        assertNull("a carriage past the bound", r.carriage(Bearer.MESSAGES_BOUND + 1))
        // the bound on what is held at once, reached by honest-looking
        // packets: each is fine and the sum is not
        val big = Bearer.Reassembly()
        val packet = ByteArray(Bearer.HEADER + 1024)
        var refused: String? = null
        var n = 0
        while (refused == null && n < 40_000) {
            packet[2] = (n ushr 8).toByte()
            packet[3] = n.toByte()
            refused = big.take(packet.copyOf())
            n++
        }
        assertEquals("more bytes than the bound allows", refused)
    }

    @Test
    fun a_truncated_set_is_incomplete_rather_than_short() {
        // the last message's packets never arrive, so the end flag never
        // does: a receiver told nothing about the count cannot mistake what
        // it has for all of it
        val wire = Wire(20)
        val messages = carriage(40, 40, 40)
        Bearer.carry(wire, messages)
        val r = Bearer.Reassembly()
        val lastMessage = wire.sent.filter { (it[0].toInt() and 0xff) == 2 }
        for (p in wire.sent - lastMessage.toSet()) assertNull(r.take(p))
        assertNull("two of three is not the set", r.carriage())
        for (p in lastMessage) assertNull(r.take(p))
        val back = r.carriage()!!
        assertEquals(3, back.size)
        for (i in messages.indices) assertArrayEquals(messages[i], back[i])
    }

    @Test
    fun one_phase_cannot_complete_another_whatever_the_indices_say() {
        // two phases carry message 0 with identical indices: the nibble is
        // the only thing telling them apart, which is the point of it
        val wire = Wire(20)
        val intent = carriage(120)
        val outcomes = carriage(60)
        Bearer.carry(wire, intent, 0)
        val boundary = wire.sent.size
        Bearer.carry(wire, outcomes, 1)
        val r = Bearer.Reassembly()
        // only the second phase's packets arrive
        for (p in wire.sent.drop(boundary)) assertNull(r.take(p))
        assertNull("phase 0 never arrived", r.carriage(0))
        assertArrayEquals(outcomes[0], r.carriage(1)!![0])
        // and the first phase's, late: each completes its own and only its own
        for (p in wire.sent.take(boundary)) assertNull(r.take(p))
        assertArrayEquals(intent[0], r.carriage(0)!![0])
        assertEquals(1, r.carriage(1)!!.size)
    }

    @Test
    fun a_link_that_will_not_send_is_reported_rather_than_assumed() {
        val dead = object : Bearer.Link {
            override fun mtu() = 20
            override fun send(packet: ByteArray) = false
        }
        assertTrue(!Bearer.carry(dead, carriage(100)))
    }

    @Test
    fun a_slice_of_no_bytes_is_not_free() {
        // the entries that index slices cost memory whether or not the
        // slices carry bytes, and a peer sending one-byte slices at fresh
        // indices is asking for entries: the bound counts them
        val r = Bearer.Reassembly()
        val packet = ByteArray(Bearer.HEADER + 1)
        var refused: String? = null
        var n = 0
        while (refused == null && n < Bearer.MESSAGE_BOUND) {
            // 16 phases x 256 messages x 65,536 slots, walked in order
            packet[0] = (n / 65_536 % 256).toByte()
            packet[1] = ((n / 65_536 / 256) shl 4).toByte()
            packet[2] = (n ushr 8).toByte()
            packet[3] = n.toByte()
            refused = r.take(packet.copyOf())
            n++
        }
        assertEquals("more bytes than the bound allows", refused)
        assertTrue(
            "refused after $n entries, not after a payload byte each",
            n <= Bearer.MESSAGE_BOUND / Bearer.ENTRY_COST + 1,
        )
        // and an empty slice that is not a message's last was never sent
        val empty = ByteArray(Bearer.HEADER)
        assertEquals("an empty slice that ends nothing", Bearer.Reassembly().take(empty))
        // where it is the last, it is the one way a zero-length message
        // arrives, and it is taken
        empty[1] = 1
        assertNull(Bearer.Reassembly().take(empty))
    }

    @Test
    fun nothing_follows_a_message_whose_last_slice_is_known() {
        val wire = Wire(20)
        Bearer.carry(wire, carriage(40))
        val r = Bearer.Reassembly()
        for (p in wire.sent) assertNull(r.take(p))
        // a slice past the last: index 9 on a message that ended at 2
        val past = ByteArray(Bearer.HEADER + 3)
        past[3] = 9
        assertEquals("a slice past the message's last", r.take(past))
        // a second, different last
        val second = ByteArray(Bearer.HEADER + 3)
        second[1] = 1
        second[3] = 1
        assertEquals("a second last slice", r.take(second))
        // the message itself is whole and unchanged
        assertArrayEquals(carriage(40)[0], r.carriage(0)!![0])
    }
}
