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
