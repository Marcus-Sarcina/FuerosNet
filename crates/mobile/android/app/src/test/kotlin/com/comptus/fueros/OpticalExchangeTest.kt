package com.comptus.fueros

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class OpticalExchangeTest {
    private fun bytes(n: Int, seed: Int) = ByteArray(n) { ((it * 7 + seed) and 0xff).toByte() }

    /** Two devices facing each other: each reads whatever the other shows,
     *  and the two advance in step until both hold everything. */
    private fun run(a: OpticalExchange, b: OpticalExchange): Int {
        var rounds = 0
        while (!(a.done() && b.done()) && rounds < 100) {
            // each camera reads the other's current code; the order within a
            // round does not matter, and a frame read twice is a duplicate
            val fa = a.frame(); val fb = b.frame()
            b.take(fa); a.take(fb)
            b.take(fa)
            rounds++
        }
        return rounds
    }

    @Test
    fun a_two_kilobyte_object_crosses_in_eight_parts_each_way_in_step() {
        val ma = bytes(2022, 1); val mb = bytes(2022, 9)
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, ma)
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, mb)
        assertEquals(8, a.count)
        assertEquals(0, a.showing())
        val rounds = run(a, b)
        assertTrue("done in $rounds rounds: two reads per part, the part and the header that says it landed", rounds <= 2 * 8 + 2)
        assertArrayEquals(mb, a.theirs())
        assertArrayEquals(ma, b.theirs())
        assertTrue(a.done() && b.done())
    }

    @Test
    fun a_device_shows_the_part_the_other_side_needs_next_and_keeps_the_last_up() {
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, bytes(600, 1))  // 3 parts
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, bytes(300, 2))  // 2 parts
        assertEquals(0, a.showing())
        // a reads b's first; a still shows its first, since b has none of a's
        assertEquals(OpticalExchange.Took.ACCEPTED, a.take(b.frame()))
        assertEquals(0, a.showing())
        // b reads a's first, whose header says a holds one of b's: b shows its second
        assertEquals(OpticalExchange.Took.ACCEPTED, b.take(a.frame()))
        assertEquals(1, b.showing())
        // a reads b's second: all of b's held, and b's header says it holds one of a's
        assertEquals(OpticalExchange.Took.COMPLETE, a.take(b.frame()))
        assertEquals("b holds one of mine: show my second", 1, a.showing())
        assertFalse(a.done())
        assertEquals(OpticalExchange.Took.ACCEPTED, b.take(a.frame()))
        assertEquals("b keeps its last part up", 1, b.showing())
        // a reads b's last again: a duplicate part, but the header moved b's count to 2
        assertEquals(OpticalExchange.Took.DUPLICATE, a.take(b.frame()))
        assertEquals(2, a.showing())
        assertEquals(OpticalExchange.Took.COMPLETE, b.take(a.frame()))
        assertTrue(b.complete())
        assertTrue("a saw b hold 2 of 3 so far", !a.done())
        assertEquals(OpticalExchange.Took.DUPLICATE, a.take(b.frame()))
        assertTrue("b's header now says it holds all three", a.done())
        assertTrue("b saw a's header say it holds both", b.done())
    }

    @Test
    fun codes_of_another_exchange_or_a_bad_shape_are_not_taken() {
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, bytes(100, 1))
        val t = OpticalExchange(OpticalExchange.TRANSCRIPT, bytes(34, 3))
        // the next exchange's code is the other side's word that it holds all of mine
        assertEquals(OpticalExchange.Took.DUPLICATE, a.take(t.frame()))
        assertEquals(OpticalExchange.Took.NOT_OURS, a.take(byteArrayOf(1, 0, 3)))
        assertEquals(OpticalExchange.Took.NOT_OURS, a.take(byteArrayOf(9, 0, 0, 1, 0)))
        assertEquals(OpticalExchange.Took.MALFORMED, a.take(byteArrayOf(1, 0, 2, 2, 0)))
        assertEquals(OpticalExchange.Took.MALFORMED, a.take(byteArrayOf(1, 0, 0, 0, 0)))
        assertNull(a.theirs())
        assertEquals(1, t.count)
        assertTrue(t.frame().size == OpticalExchange.HEADER + 34)
    }

    @Test
    fun a_code_of_the_next_exchange_counts_as_the_other_side_holding_everything() {
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, bytes(600, 1))
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, bytes(300, 2))
        // a holds all of b's; b moved to the transcript before a read its last header
        a.take(b.frame()); b.take(a.frame()); a.take(b.frame())
        assertTrue(a.complete())
        assertFalse(a.done())
        val t = OpticalExchange(OpticalExchange.TRANSCRIPT, bytes(34, 3))
        assertEquals(OpticalExchange.Took.DUPLICATE, a.take(t.frame()))
        assertTrue("b's transcript code says b has all of a's", a.done())
        // but a code of an exchange further on, or before, is not ours
        val far = OpticalExchange(2, bytes(10, 4))
        assertEquals(OpticalExchange.Took.NOT_OURS, a.take(far.frame()))
        assertEquals(OpticalExchange.Took.NOT_OURS, t.take(a.frame()))
    }

    @Test
    fun the_status_line_names_both_sides_progress() {
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, bytes(600, 1))
        assertEquals("Showing part 1 of 3; received none of theirs yet; they hold 0 of your 3.", a.status())
    }
}
