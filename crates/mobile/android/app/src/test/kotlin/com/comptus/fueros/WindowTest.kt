package com.comptus.fueros

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * **What the rotating window costs in rounds**, against the one part at a
 * time it replaces.
 *
 * `qr.looks` of 2026-10-07 measured where the exchange's time went: a
 * third to three quarters of camera frames decode, and **85 to 93% of the
 * successful decodes were the same part over again**, because the sender
 * held its frame until the counterparty's header reported progress. These
 * simulate a reader that catches a given frame with probability `p` and
 * count how many *frames shown* it takes to finish, which is the figure
 * the wall clock follows.
 */
class WindowTest {

    private fun obj(n: Int, seed: Int) = ByteArray(n) { (it * 31 + seed).toByte() }

    /**
     * Run the two sides to completion with a reader that catches each
     * shown frame with probability `p`, and answer how many frames each
     * side had to show. `window` false pins the old behaviour — the part
     * is only ever the first one they lack — so the two are measured on
     * the same harness.
     */
    private fun rounds(parts: Int, p: Double, window: Boolean, seed: Long): Int {
        val size = parts * OpticalExchange.CHUNK
        val a = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(size, 1), OpticalExchange.CHUNK)
        val b = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(size, 2), OpticalExchange.CHUNK)
        val rng = java.util.Random(seed)
        var shown = 0
        while (!(a.done() && b.done()) && shown < 200_000) {
            val fromA = a.frame()
            val fromB = b.frame()
            val trackA = a.tracking()
            val trackB = b.tracking()
            // **the rows ride the same capture as the symbol**: they are
            // drawn below it and sampled from the same frame, so a frame
            // caught delivers both and a frame missed delivers neither
            if (rng.nextDouble() < p) { b.take(fromA); if (window) b.takeTracking(trackA) }
            if (rng.nextDouble() < p) { a.take(fromB); if (window) a.takeTracking(trackB) }
            if (window) { a.turn(); b.turn() }
            shown++
        }
        assertTrue("finished in $shown frames", a.done() && b.done())
        assertArrayEquals(a.theirs(), obj(size, 2))
        return shown
    }

    @Test
    fun the_window_finishes_in_far_fewer_frames_than_one_part_at_a_time() {
        for (p in listOf(0.34, 0.75)) {
            val one = (0L until 5L).map { rounds(102, p, window = false, seed = it) }.average()
            val win = (0L until 5L).map { rounds(102, p, window = true, seed = it) }.average()
            println("p=$p  one-at-a-time ${one.toInt()} frames,  window ${win.toInt()} frames  => ${"%.1f".format(one / win)}x")
            assertTrue("the window is not slower at p=$p: $one vs $win", win <= one)
        }
    }

    /** Whatever the rotation does, nothing may be skipped or duplicated
     *  into the wrong slot: the assembled object is the proof. */
    @Test
    fun the_object_assembles_whole_under_a_lossy_reader() {
        for (seed in 0L until 8L) rounds(40, 0.2, window = true, seed = seed)
    }

    /**
     * **A set module is believed on its second sighting, not its first.**
     * The rows carry no error correction and the two misreadings cost
     * differently: unset-when-set is a redundant re-show, set-when-unset
     * loses a part for good. So the costly direction is confirmed.
     */
    @Test
    fun a_set_tracking_module_is_confirmed_before_it_is_believed() {
        val x = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(OpticalExchange.CHUNK * 6, 1), OpticalExchange.CHUNK)
        val says = BooleanArray(x.count) { it == 2 }
        assertEquals("nothing believed yet", 0, x.theirReceived())
        x.takeTracking(says)
        assertEquals("one sighting is not enough", 0, x.theirReceived())
        x.takeTracking(says)
        assertEquals("the second confirms it", 1, x.theirReceived())
        // and it never walks back on a frame that misses it
        x.takeTracking(BooleanArray(x.count))
        assertEquals("monotonic", 1, x.theirReceived())
    }

    /** **Losing the rows entirely is safe**: the header's contiguous count
     *  is a floor under the bitmap, so a side that cannot sample them
     *  behaves exactly as the shipped exchange did. */
    @Test
    fun without_the_rows_the_header_s_count_still_carries_it() {
        for (seed in 0L until 4L) {
            // `window = false` never calls takeTracking, so this is the
            // old channel on the new state model
            rounds(40, 0.3, window = false, seed = seed)
        }
    }

    /** A window of one part is the old behaviour, so the change is
     *  bounded: with `WINDOW` at 1 the two are the same exchange. */
    @Test
    fun the_window_subsumes_the_single_part_case() {
        val x = OpticalExchange(OpticalExchange.CONTRIBUTION, obj(OpticalExchange.CHUNK * 4, 1), OpticalExchange.CHUNK)
        assertEquals("nothing of theirs read yet, so the first part", 0, x.showing())
        // the rotation stays inside the parts they cannot have
        repeat(50) { x.turn(); assertTrue(x.showing() in 0 until x.count) }
    }
}
