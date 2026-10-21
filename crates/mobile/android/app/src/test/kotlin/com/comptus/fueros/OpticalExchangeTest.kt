package com.comptus.fueros

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class OpticalExchangeTest {

    /**
     * **The chunk these tests name for themselves.** They are about the
     * exchange's rules — who shows what, who advances, what a stale code
     * counts as — and not about the production constant, which is chosen for
     * reading distance and moved when that changes
     * (`OpticalExchange.CHUNK`, and `OpticalChunkTest` pins its cost).
     * Four of these broke when the constant moved from 256 to 48 on
     * 2026-10-06, which is a test reading a number it did not mean to
     * depend on.
     */
    private val PARTS_OF_256 = 256
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
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, ma, PARTS_OF_256)
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, mb, PARTS_OF_256)
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
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, bytes(600, 1), PARTS_OF_256)  // 3 parts
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, bytes(300, 2), PARTS_OF_256)  // 2 parts
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
        assertTrue(b.theirs() != null)
        assertTrue("a saw b hold 2 of 3 so far", !a.done())
        assertEquals(OpticalExchange.Took.DUPLICATE, a.take(b.frame()))
        assertTrue("b's header now says it holds all three", a.done())
        assertTrue("b saw a's header say it holds both", b.done())
    }

    @Test
    fun codes_of_another_exchange_or_a_bad_shape_are_not_taken() {
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, bytes(100, 1), PARTS_OF_256)
        val t = OpticalExchange(OpticalExchange.TRANSCRIPT, bytes(34, 3), PARTS_OF_256)
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
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, bytes(600, 1), PARTS_OF_256)
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, bytes(300, 2), PARTS_OF_256)
        // a holds all of b's; b moved to the transcript before a read its last header
        a.take(b.frame()); b.take(a.frame()); a.take(b.frame())
        assertTrue(a.theirs() != null)
        assertFalse(a.done())
        val t = OpticalExchange(OpticalExchange.TRANSCRIPT, bytes(34, 3), PARTS_OF_256)
        assertEquals(OpticalExchange.Took.DUPLICATE, a.take(t.frame()))
        assertTrue("b's transcript code says b has all of a's", a.done())
        // but a code of an exchange further on, or before, is not ours
        val far = OpticalExchange(2, bytes(10, 4), PARTS_OF_256)
        assertEquals(OpticalExchange.Took.NOT_OURS, a.take(far.frame()))
        assertEquals(OpticalExchange.Took.NOT_OURS, t.take(a.frame()))
    }

    @Test
    fun the_status_line_names_both_sides_progress() {
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, bytes(600, 1), PARTS_OF_256)
        assertEquals("Showing part 1 of 3; received none of theirs yet; they hold 0 of your 3.", a.status())
    }

    /**
     * **A side reads its own code back off the other phone's glass.** The
     * selfie camera faces a screen, and a screen reflects; ZXing decodes a
     * mirrored symbol. Taken as the other side's, the reflection's header
     * says "they hold everything" the moment this side does — on the
     * bench one TCL declared the exchange done at 102/102 while the other
     * held 80, and starved [2026-10-10]. A part equal to this side's own at
     * that index can only be this side's, so it is named and nothing in it
     * is believed.
     */
    @Test
    fun a_reflection_of_this_sides_own_code_is_named_and_changes_nothing() {
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, bytes(600, 1), PARTS_OF_256)  // 3 parts
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, bytes(600, 2), PARTS_OF_256)  // 3 parts, other bytes
        // b has given a everything, so a's own header carries received = 3
        repeat(3) { a.take(b.frame()); b.turn() }
        assertEquals(3, a.received())
        val own = a.frame()
        assertTrue("this side knows its own code", a.mine(own))
        assertFalse("and does not mistake the other side's for it", a.mine(b.frame()))

        // the reflection, read by this side: named, and no figure moves
        val theyHeldBefore = a.theirReceived()
        assertEquals(OpticalExchange.Took.MIRROR, a.take(own))
        assertEquals("their holdings did not move on this side's own header", theyHeldBefore, a.theirReceived())
        assertFalse("and the exchange is not done for having read itself", a.done())

        // the other side is in the same position and names a's code as a's
        assertEquals(OpticalExchange.Took.MIRROR, b.take(b.frame()))
        assertTrue(b.mine(b.frame()))
        assertFalse(b.mine(own))

        // **the transcript is the same bytes on both phones**, so there the
        // rule would name every genuine code a reflection — and did, on
        // the bench: both phones sat on 1/2 reading each other. It is not
        // asked of the transcript, where a reflection is also harmless
        val ta = OpticalExchange(OpticalExchange.TRANSCRIPT, bytes(36, 9), PARTS_OF_256)
        val tb = OpticalExchange(OpticalExchange.TRANSCRIPT, bytes(36, 9), PARTS_OF_256)
        assertFalse("identical transcripts are not mistaken for reflections", ta.mine(tb.frame()))
        // one part under this chunk, so the first taken is the last
        assertEquals(OpticalExchange.Took.COMPLETE, ta.take(tb.frame()))
        assertEquals(1, ta.received())
    }

    /** **The exchange can complete with neither count moving.** The
     *  tracking rows can confirm that the other side holds every part of
     *  mine before its header says so, and `done()` waits for the header
     *  alone (the rows cannot say *all*); so the header that completes the
     *  exchange may arrive on a frame whose `received()` and
     *  `theirReceived()` are what they already were. A screen that judges
     *  news by the two counts alone misses it — and the camera then
     *  declines every identical frame as not news (GXV × 4VC,
     *  2026-10-11). The transcript is the case: identical on both sides,
     *  two parts each. */
    @Test
    fun the_exchange_can_complete_on_a_frame_that_moves_neither_count() {
        val id = bytes(36, 9)
        val a = OpticalExchange(1, id, chunk = 18)
        val b = OpticalExchange(1, id, chunk = 18)
        assertEquals(2, a.count)
        // the two read each other's first parts; each side shows the
        // lowest part the other is known to lack, so a's header (got = 1)
        // is what moves b on to its second part, and the reverse
        assertEquals(OpticalExchange.Took.ACCEPTED, a.take(b.frame()))
        assertEquals(OpticalExchange.Took.ACCEPTED, b.take(a.frame()))
        assertEquals(OpticalExchange.Took.COMPLETE, a.take(b.frame()))
        assertEquals(2, a.received())
        // b's rows say it holds both of a's (confirmed twice); b's header
        // still says one, since b has not read a's second part
        repeat(Tracking.CONFIRM) { a.takeTracking(booleanArrayOf(true, true)) }
        assertEquals(2, a.theirReceived())
        assertFalse("the rows cannot say all: done waits for the header", a.done())
        // now b really takes a's second part, and its header says got = 2
        assertEquals(OpticalExchange.Took.COMPLETE, b.take(a.frame()))
        val heldBefore = a.received()
        val theyHeldBefore = a.theirReceived()
        val v = a.version()
        val took = a.take(b.frame())
        assertEquals("the part was already held", OpticalExchange.Took.DUPLICATE, took)
        assertEquals(heldBefore, a.received())
        assertEquals(theyHeldBefore, a.theirReceived())
        assertTrue("and yet the exchange is now complete", a.done())
        assertTrue("which the version says, where the counts could not", a.version() > v)
        // and the same frame again is not news of any kind
        val v2 = a.version()
        assertEquals(OpticalExchange.Took.DUPLICATE, a.take(b.frame()))
        assertEquals(v2, a.version())
    }

    /** **Another ceremony's code of this type is not ours**, not malformed:
     *  once the other side's count is known, a frame with a different
     *  count is some other phone's exchange, read by accident, and nothing
     *  of this one is wrong. A frame that cannot be a frame at all is. */
    @Test
    fun another_ceremonys_code_is_not_ours_and_a_broken_one_is_malformed() {
        val a = OpticalExchange(0, bytes(40, 1), chunk = 20)
        val b = OpticalExchange(0, bytes(40, 2), chunk = 20)
        assertEquals(OpticalExchange.Took.ACCEPTED, a.take(b.frame()))
        val stray = byteArrayOf(1, 0, 0, 3, 0) + bytes(20, 9)
        assertEquals(OpticalExchange.Took.NOT_OURS, a.take(stray))
        assertEquals(OpticalExchange.Took.MALFORMED, a.take(byteArrayOf(1, 0, 5, 2, 0) + bytes(20, 9)))
        assertEquals(OpticalExchange.Took.MALFORMED, a.take(byteArrayOf(1, 0, 0, 0, 0)))
    }
}

/**
 * **The trace the witnesses are owed** (`wire-format.md` §7.10.2, kind 21)
 * [author, 2026-10-19]: the exchange reports its own shape as it moves —
 * both counts once they are known, each inbound part as it lands, each
 * outbound part the other side's display acknowledges — and never its
 * content, since a part is half of what keys the local session.
 */
class OpticalTraceTest {

    private fun bytes(n: Int, seed: Int) = ByteArray(n) { ((it * 7 + seed) and 0xff).toByte() }

    @Test
    fun the_shape_comes_first_and_then_one_event_per_part() {
        val a = OpticalExchange(0, bytes(60, 1), chunk = 20)
        val b = OpticalExchange(0, bytes(40, 2), chunk = 20)
        assertEquals(3, a.count)
        assertEquals(2, b.count)
        // nothing is owed before a code of theirs has been read
        assertTrue(a.drainTrace().isEmpty())
        // their first code: both counts are known, and the part it carried
        a.take(b.frame())
        val first = a.drainTrace()
        assertEquals(
            "the shape precedes the part it arrived with",
            OpticalExchange.Event.Shape(mine = 3, theirs = 2),
            first.first(),
        )
        assertTrue(
            "and the part itself, by index: $first",
            first.drop(1).all { it is OpticalExchange.Event.Inbound },
        )
        // **drained, so nothing is sent twice**: the flow hands each event
        // to the kernel as it happens, and the exchange forgets it
        assertTrue(a.drainTrace().isEmpty())
    }

    @Test
    fun an_acknowledgement_of_this_sides_parts_is_one_event_each() {
        val a = OpticalExchange(0, bytes(60, 1), chunk = 20)
        val b = OpticalExchange(0, bytes(60, 2), chunk = 20)
        // b reads two of a's parts, so a's next read of b's header says so
        b.take(a.frame())
        b.take(a.frame())
        a.drainTrace()
        a.take(b.frame())
        val events = a.drainTrace()
        val out = events.filterIsInstance<OpticalExchange.Event.Outbound>()
        assertEquals("one event per index newly acknowledged", out.size, out.map { it.index }.distinct().size)
        assertTrue("and the indices are this side's own: $out", out.all { it.index < a.count })
    }

    @Test
    fun a_whole_exchange_traces_every_part_it_moved_and_no_content() {
        val a = OpticalExchange(0, bytes(100, 1), chunk = 20)
        val b = OpticalExchange(0, bytes(100, 2), chunk = 20)
        val seen = mutableListOf<OpticalExchange.Event>()
        var rounds = 0
        while (!(a.done() && b.done()) && rounds < 100) {
            val fa = a.frame()
            val fb = b.frame()
            b.take(fa)
            a.take(fb)
            seen += a.drainTrace()
            rounds++
        }
        assertTrue("the exchange completed", a.done())
        // every part of theirs landed exactly once, by index
        val inbound = seen.filterIsInstance<OpticalExchange.Event.Inbound>().map { it.index }
        assertEquals(b.count, inbound.distinct().size)
        assertEquals("never twice", inbound.size, inbound.distinct().size)
        // one shape, no more
        assertEquals(1, seen.count { it is OpticalExchange.Event.Shape })
        // **nothing in the trace is content**: the events carry counts and
        // indices, which is the whole of what a witness is told
        assertTrue(
            seen.all {
                it is OpticalExchange.Event.Shape ||
                    it is OpticalExchange.Event.Inbound ||
                    it is OpticalExchange.Event.Outbound
            },
        )
    }
}
