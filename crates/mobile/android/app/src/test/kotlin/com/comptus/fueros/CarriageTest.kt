package com.comptus.fueros

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The bearer choice and one carriage across it, on the JVM. */
class CarriageTest {

    private class Pipe(val mtu: Int = 23 - 3) : Bearer.Link {
        var other: Carriage? = null
        var dead = false
        override fun mtu() = mtu
        override fun send(packet: ByteArray): Boolean {
            if (dead) return false
            other?.packet(packet)
            return true
        }
    }

    @Test
    fun a_radio_is_preferred_and_its_absence_is_named_rather_than_implied() {
        assertEquals(Carriage.Chosen.RADIO, Carriage(Pipe()).chosen)
        // §14.3.1's last resort is not built, and this says so instead of
        // reporting a bearer it does not have
        assertEquals(Carriage.Chosen.NONE, Carriage(null).chosen)
        assertFalse("nothing is carried over no bearer", Carriage(null).send(listOf(ByteArray(4))))
    }

    @Test
    fun a_carriage_set_crosses_in_order_and_the_count_is_what_completes_it() {
        val pipe = Pipe()
        val near = Carriage(pipe)
        val far = Carriage(Pipe())
        pipe.other = far
        // an exchange and two continuations, the shapes §14.3.2 gives
        val set = listOf(
            ByteArray(90) { it.toByte() },
            ByteArray(4000) { (it * 3).toByte() },
            ByteArray(1200) { (it * 5).toByte() },
        )
        assertNull("nothing sent is nothing to take", far.received())
        assertTrue(near.send(set))
        val back = far.received()
        assertNotNull("the sender flagged its last message", back)
        assertEquals(set.size, back!!.size)
        for (i in set.indices) assertArrayEquals("message $i", set[i], back[i])
    }

    @Test
    fun the_capture_key_phase_is_assembled_apart_from_the_others() {
        val pipe = Pipe()
        val near = Carriage(pipe)
        val far = Carriage(Pipe())
        pipe.other = far
        // the shape of a `CaptureKeyHandover`: an array head, a version,
        // and two 32-byte strings, 70 bytes of which this file reads none
        val handover = ByteArray(70) { (it * 7).toByte() }
        assertTrue(near.send(listOf(handover), Carriage.Phase.CAPTURE_KEY))
        assertNull("not the intent's", far.received(Carriage.Phase.INTENT))
        assertNull("not the outcomes'", far.received(Carriage.Phase.PROXIMITY))
        assertNull("not the prekeys'", far.received(Carriage.Phase.PREKEY))
        val back = far.received(Carriage.Phase.CAPTURE_KEY)
        assertNotNull("the one message of its phase", back)
        assertEquals(1, back!!.size)
        assertArrayEquals(handover, back[0])
    }

    @Test
    fun a_link_that_stops_mid_carriage_leaves_unsent_work_reported() {
        val pipe = Pipe()
        val far = Carriage(Pipe())
        pipe.other = far
        pipe.dead = true
        assertFalse(Carriage(pipe).send(listOf(ByteArray(100))))
        assertNull(far.received())
    }
}

/** The capture key phase is let go of, on both sides of the carriage. */
class CarriageWipeTest {

    private class Pipe(val mtu: Int = 23 - 3) : Bearer.Link {
        var other: Carriage? = null
        val sent = ArrayList<ByteArray>()
        override fun mtu() = mtu
        override fun send(packet: ByteArray): Boolean {
            other?.packet(packet)
            return sent.add(packet)
        }
    }

    @Test
    fun the_capture_key_phase_is_wiped_once_sent_and_discarded_once_taken() {
        val pipe = Pipe()
        val near = Carriage(pipe)
        val far = Carriage(Pipe())
        pipe.other = far
        val handover = ByteArray(70) { (it * 7 + 1).toByte() }
        assertTrue(near.send(listOf(handover), Carriage.Phase.CAPTURE_KEY, wipe = true))
        for (p in pipe.sent) assertTrue("sent packets are zeroed", p.all { it == 0.toByte() })
        val back = far.received(Carriage.Phase.CAPTURE_KEY)
        assertNotNull(back)
        assertArrayEquals(handover, back!![0])
        far.discard(Carriage.Phase.CAPTURE_KEY)
        assertNull("nothing of the phase remains to take", far.received(Carriage.Phase.CAPTURE_KEY))
        assertArrayEquals("what was taken is the taker's to wipe", handover, back[0])
    }
}

/** The conversation's carriages over the bearer: one message per phase,
 *  taken as each lands, the phases wrapping without collision. */
class ConversationCarriageTest {

    private class Pipe(val mtu: Int = 23 - 3) : Bearer.Link {
        var other: Carriage? = null
        override fun mtu() = mtu
        override fun send(packet: ByteArray): Boolean {
            other?.packet(packet)
            return true
        }
    }

    @Test
    fun each_conversation_carriage_takes_a_phase_of_its_own_and_the_range_wraps() {
        val first = Carriage.Phase.conversation(0)
        assertEquals(Carriage.Phase.CONVERSATION, first)
        // the three staged phases are never reused by the conversation
        for (n in 0 until 3 * Carriage.Phase.CONVERSATIONS) {
            val ph = Carriage.Phase.conversation(n)
            assertTrue("phase $ph for carriage $n is past the staged ones", ph >= Carriage.Phase.CONVERSATION)
            assertTrue("phase $ph fits the header's nibble", ph < Bearer.PHASES)
        }
        // and the count wraps round the range rather than running off it
        assertEquals(first, Carriage.Phase.conversation(Carriage.Phase.CONVERSATIONS))
    }

    @Test
    fun a_message_per_phase_is_whole_on_its_own_and_a_discarded_phase_takes_the_next_round() {
        val a = Pipe(); val b = Pipe()
        val ca = Carriage(a); val cb = Carriage(b)
        a.other = cb; b.other = ca
        // more messages than the range has phases: the receiver takes and
        // discards each as it lands, so the wrap lands on a cleared phase
        for (n in 0 until Carriage.Phase.CONVERSATIONS + 2) {
            val ph = Carriage.Phase.conversation(n)
            val msg = ByteArray(40) { (n + it).toByte() }
            assertTrue(ca.send(listOf(msg), ph))
            val set = cb.received(ph)
            assertNotNull("carriage $n is whole on its own in phase $ph", set)
            assertArrayEquals(msg, set!![0])
            cb.discard(ph)
            assertNull("a discarded phase holds nothing", cb.received(ph))
        }
    }
}
